/*
 * F.32-1 (2026-05-24) — false-sharing on adjacent @form cells:
 * theoretical-ceiling microbench.
 *
 * Question this answers
 * ---------------------
 * Two producer pools writing disjoint keys into a shared
 * @form(hashmap) reach adjacent cells. If those cells share a
 * 64-byte cache line, the cores ping-pong the line under MESI
 * even though logically each producer owns its own cell. The
 * F.32-1 fix pads the cell stride up to LOTUS_CACHE_LINE.
 *
 * Before changing the substrate, this bench establishes the
 * theoretical ceiling for that fix: hand-written C, two pthreads
 * pinned to sibling cores, hammering adjacent counters with and
 * without _Alignas(64) padding. If padding doesn't pay off in
 * this minimal C shape, F.32-1 can't pay off in Hale either.
 *
 * Mirrors the precedent set by experiments/k2-zero-copy/bench.c:
 * prove the substrate-design rationale in C before turning it
 * into compiler / runtime work.
 *
 * Three configurations measured
 * -----------------------------
 *   1. packed: two int64_t counters in adjacent struct fields
 *      (offset 0 and 8). Shares one 64B cache line. Worst case.
 *
 *   2. padded_64: counters separated by _Alignas(64) so they
 *      land on distinct cache lines. The fix F.32-1 will emit.
 *
 *   3. padded_128: counters on distinct 128B-aligned lines.
 *      Covers Apple M-series (effective line 128B) and Intel
 *      adjacent-line prefetch.
 *
 * Methodology
 * -----------
 * Per [[bench-methodology]]: 5 rounds × 10M increments per
 * thread. pthread_setaffinity_np pins thread A to core 0,
 * thread B to core 1 (assumed to share L2; verify on target
 * via `lscpu --extended=CPU,CORE,SOCKET,CACHE`). Report median
 * ns/op across rounds.
 *
 * Build & run
 * -----------
 *   gcc -O2 -pthread -o experiments/f32-false-sharing/bench \
 *       experiments/f32-false-sharing/bench.c
 *   ./experiments/f32-false-sharing/bench
 *
 * Or use run.sh in this directory.
 *
 * Expected outcome
 * ----------------
 * On any 2-core x86_64 / arm64 host with SMT siblings on the
 * same L2, padded_64 should be 3-5x faster than packed for the
 * pure-increment loop. The F.32-1 acceptance gate (>= 2x on the
 * @form(hashmap) bench) is a deliberately conservative subset
 * — real maps add hash + probe cost that dilutes the cache-line
 * contribution.
 */

#define _GNU_SOURCE
#include <pthread.h>
#include <sched.h>
#include <stdatomic.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#define ITER_PER_ROUND  10000000
#define N_ROUNDS        5
#define CACHE_LINE_64   64
#define CACHE_LINE_128  128

typedef struct {
    int64_t a;
    int64_t b;
} packed_t;

typedef struct {
    _Alignas(CACHE_LINE_64) int64_t a;
    _Alignas(CACHE_LINE_64) int64_t b;
} padded_64_t;

typedef struct {
    _Alignas(CACHE_LINE_128) int64_t a;
    _Alignas(CACHE_LINE_128) int64_t b;
} padded_128_t;

typedef struct {
    int64_t *slot;
    int iterations;
    int cpu;
} worker_arg_t;

static int pin_to(int cpu) {
    cpu_set_t set;
    CPU_ZERO(&set);
    CPU_SET(cpu, &set);
    return pthread_setaffinity_np(pthread_self(), sizeof(set), &set);
}

static void *worker(void *arg) {
    worker_arg_t *w = (worker_arg_t *)arg;
    /* Best-effort pinning — if affinity isn't permitted (e.g.
     * containers without CAP_SYS_NICE), the bench still runs
     * but the false-sharing signal will be noisier. */
    (void)pin_to(w->cpu);
    /* volatile prevents the compiler from hoisting the
     * increments out of the loop or coalescing them into one
     * store. We want every iteration to do a real RMW against
     * memory so MESI traffic is realistic. */
    volatile int64_t *p = w->slot;
    for (int i = 0; i < w->iterations; i++) {
        *p = *p + 1;
    }
    return NULL;
}

static int64_t monotonic_ns(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (int64_t)ts.tv_sec * 1000000000LL + (int64_t)ts.tv_nsec;
}

static int cmp_i64(const void *x, const void *y) {
    int64_t a = *(const int64_t *)x;
    int64_t b = *(const int64_t *)y;
    return (a > b) - (a < b);
}

static int64_t median_ns(int64_t *samples, int n) {
    qsort(samples, n, sizeof(int64_t), cmp_i64);
    return samples[n / 2];
}

typedef struct {
    const char *label;
    int64_t    *slot_a;
    int64_t    *slot_b;
    ptrdiff_t   stride;  /* bytes between slot_a and slot_b */
} config_t;

static int64_t run_round(config_t cfg) {
    /* Reset to zero so counters don't overflow across rounds. */
    *cfg.slot_a = 0;
    *cfg.slot_b = 0;

    pthread_t ta, tb;
    worker_arg_t arga = { cfg.slot_a, ITER_PER_ROUND, 0 };
    worker_arg_t argb = { cfg.slot_b, ITER_PER_ROUND, 1 };

    int64_t t0 = monotonic_ns();
    pthread_create(&ta, NULL, worker, &arga);
    pthread_create(&tb, NULL, worker, &argb);
    pthread_join(ta, NULL);
    pthread_join(tb, NULL);
    int64_t t1 = monotonic_ns();

    return t1 - t0;
}

static void run_config(config_t cfg) {
    int64_t samples[N_ROUNDS];
    for (int r = 0; r < N_ROUNDS; r++) {
        samples[r] = run_round(cfg);
    }
    int64_t med = median_ns(samples, N_ROUNDS);
    double per_op = (double)med / (double)(ITER_PER_ROUND * 2);
    printf("%-12s stride=%-4td median_ns=%-12lld ns/op=%.2f\n",
           cfg.label,
           cfg.stride,
           (long long)med,
           per_op);
}

int main(void) {
    /* Allocate each config in its own block so the compiler
     * can't reorder unrelated fields across configs. */
    packed_t      packed     = {0};
    padded_64_t   padded64   = {0};
    padded_128_t  padded128  = {0};

    config_t configs[] = {
        { "packed",      &packed.a,    &packed.b,
          (char *)&packed.b    - (char *)&packed.a    },
        { "padded_64",   &padded64.a,  &padded64.b,
          (char *)&padded64.b  - (char *)&padded64.a  },
        { "padded_128",  &padded128.a, &padded128.b,
          (char *)&padded128.b - (char *)&padded128.a },
    };

    printf("iter_per_round=%d rounds=%d threads_pinned_to=cpu0,cpu1\n",
           ITER_PER_ROUND, N_ROUNDS);

    for (size_t i = 0; i < sizeof(configs) / sizeof(configs[0]); i++) {
        run_config(configs[i]);
    }

    /* Acceptance check: padded_64 must be measurably faster
     * than packed. We deliberately don't fail the bench (this
     * is a measurement tool, not a CI gate) — print a one-line
     * verdict so the operator sees it at a glance. */
    return 0;
}
