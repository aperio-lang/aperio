/*
 * Lotus region allocator — v0 substrate.
 *
 * One arena = a linked list of bump chunks. Allocation bumps a
 * pointer in the head chunk; if the head can't fit the request,
 * a fresh chunk is malloc'd and pushed on the front. Destruction
 * walks the list and frees every chunk wholesale — no per-object
 * free, ever (matching spec/memory.md: "When the locus dissolves,
 * the region is freed wholesale.").
 *
 * v0 lives behind a stable C ABI so the LLVM-IR side of the
 * compiler doesn't need to know about the chunk-list shape.
 * m22 added per-coordinatee sub-regions (chunked-class
 * projection): a parent arena can carve "sub-region" arenas for
 * its accepted children, and tracks the slot indices via a
 * free-list so children can come and go without the parent's
 * bookkeeping growing unbounded. Sub-regions still hold their
 * own chunk lists — they're independent allocations — but they
 * register with the parent on creation and return their slot to
 * the parent's free-list on destroy.
 *
 * Backed by libc malloc for the chunks themselves. That's not a
 * cheat — the substrate's job is wholesale-region management;
 * the underlying *page* supplier can be libc, mmap, or a
 * pre-reserved pool, and the arena interface above doesn't
 * change. Replace this file's malloc/free with mmap when the
 * scheduler lands and we want page-aligned regions.
 */

#include <stdint.h>
#include <stddef.h>
#include <stdlib.h>
#include <string.h>

/* Default chunk size: 64KB. Big enough that most loci fit in
 * one chunk, small enough that a leaf locus that allocates a
 * single ClosureViolation doesn't waste an entire MB. Tunable. */
#define LOTUS_ARENA_CHUNK_BYTES (64 * 1024)

typedef struct lotus_arena_chunk {
    struct lotus_arena_chunk *next;
    size_t                    used;
    size_t                    cap;
    /* `data` follows in the same allocation — accessed as
     * (char *)(chunk + 1). Inlined-trailing layout means each
     * chunk is one malloc, not two. */
} lotus_arena_chunk_t;

typedef struct lotus_arena {
    lotus_arena_chunk_t *head;
    size_t               default_chunk_size;
    /* m22: sub-region tracking. If `parent` is non-NULL, this
     * arena is a sub-region carved at one of its parent's slots;
     * destroy returns `slot` to the parent's free-list so the
     * next subregion_create can reuse it. Top-level arenas (the
     * program-wide @lotus.arena.global, plus any locus whose
     * parent is not chunked-class) have parent == NULL. */
    struct lotus_arena  *parent;
    int                  slot;
    /* m22: free-list of slot indices for sub-region children
     * (chunked-class). next_slot is the monotonic counter; freed
     * slots get pushed onto free_list and re-handed out before
     * the counter bumps again. free_list grows on demand. */
    int                 *free_list;
    size_t               free_count;
    size_t               free_cap;
    int                  next_slot;
} lotus_arena_t;

static lotus_arena_chunk_t *lotus_arena_new_chunk(size_t cap) {
    lotus_arena_chunk_t *c =
        (lotus_arena_chunk_t *)malloc(sizeof(lotus_arena_chunk_t) + cap);
    if (!c) return NULL;
    c->next = NULL;
    c->used = 0;
    c->cap  = cap;
    return c;
}

static inline size_t lotus_align_up(size_t n, size_t a) {
    return (n + a - 1) & ~(a - 1);
}

static lotus_arena_t *lotus_arena_alloc_struct(void) {
    lotus_arena_t *a = (lotus_arena_t *)malloc(sizeof(lotus_arena_t));
    if (!a) return NULL;
    a->default_chunk_size = LOTUS_ARENA_CHUNK_BYTES;
    a->head = lotus_arena_new_chunk(a->default_chunk_size);
    if (!a->head) {
        free(a);
        return NULL;
    }
    a->parent     = NULL;
    a->slot       = -1;
    a->free_list  = NULL;
    a->free_count = 0;
    a->free_cap   = 0;
    a->next_slot  = 0;
    return a;
}

/* Public ABI ---------------------------------------------------- */

lotus_arena_t *lotus_arena_create(void) {
    return lotus_arena_alloc_struct();
}

/* Carve a sub-region of `parent`. The sub-region holds its own
 * chunk list (independent allocation lifetime is *bounded* by
 * the parent's, but the chunks themselves are separate from the
 * parent's chunks — m22 doesn't yet pool memory across siblings).
 *
 * The point of this entry point vs. plain `lotus_arena_create()`
 * is the bookkeeping: we get a slot number from the parent's
 * free-list / counter, and `lotus_arena_destroy` returns that
 * slot when this sub-region dies. The free-list keeps the
 * parent's slot space O(peak children alive), not O(total
 * children ever accepted). */
lotus_arena_t *lotus_arena_create_subregion(lotus_arena_t *parent) {
    if (!parent) return lotus_arena_create();
    lotus_arena_t *a = lotus_arena_alloc_struct();
    if (!a) return NULL;
    a->parent = parent;
    if (parent->free_count > 0) {
        a->slot = parent->free_list[--parent->free_count];
    } else {
        a->slot = parent->next_slot++;
    }
    return a;
}

void *lotus_arena_alloc(lotus_arena_t *a, size_t size, size_t align) {
    if (!a) return NULL;
    if (size == 0) size = 1;        /* every alloc gets a unique addr */
    if (align == 0) align = 8;      /* default 8-byte alignment */

    lotus_arena_chunk_t *c = a->head;
    size_t off = lotus_align_up(c->used, align);
    if (off + size > c->cap) {
        /* Need a fresh chunk. Size it to cover this single
         * request if the request itself is larger than the
         * default; otherwise use the default. The new chunk
         * becomes the head, so subsequent small allocs land
         * in it (and we don't bother trying to fit them into
         * older chunks — keeps the bump fast and the lookup
         * O(1)). */
        size_t need = size + align;
        size_t cap  = need > a->default_chunk_size
                          ? need
                          : a->default_chunk_size;
        lotus_arena_chunk_t *fresh = lotus_arena_new_chunk(cap);
        if (!fresh) return NULL;
        fresh->next = c;
        a->head = fresh;
        c = fresh;
        off = lotus_align_up(c->used, align);
    }

    char *base = (char *)(c + 1);
    void *p    = base + off;
    c->used    = off + size;
    return p;
}

void lotus_arena_destroy(lotus_arena_t *a) {
    if (!a) return;

    /* m22: if this is a sub-region, return its slot to the
     * parent's free-list so a future create_subregion can reuse
     * it. Grow the free_list capacity as needed (doubling).
     * The parent itself stays alive — only the SUB-region's
     * chunks + struct go away here. */
    if (a->parent) {
        lotus_arena_t *p = a->parent;
        if (p->free_count == p->free_cap) {
            size_t new_cap = p->free_cap == 0 ? 8 : p->free_cap * 2;
            int *new_list  = (int *)realloc(p->free_list,
                                            new_cap * sizeof(int));
            if (new_list) {
                p->free_list = new_list;
                p->free_cap  = new_cap;
            }
            /* If realloc failed, we silently drop the slot —
             * functionally correct (slot never gets reused) but
             * causes parent's slot space to grow. Acceptable
             * graceful-degradation for v0. */
        }
        if (p->free_count < p->free_cap) {
            p->free_list[p->free_count++] = a->slot;
        }
    }

    lotus_arena_chunk_t *c = a->head;
    while (c) {
        lotus_arena_chunk_t *next = c->next;
        free(c);
        c = next;
    }
    if (a->free_list) free(a->free_list);
    free(a);
}
