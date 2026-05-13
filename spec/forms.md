# Forms

A **form** is a compiler-recognized annotation on a locus
declaration that picks an efficient lowering for the locus's
storage and synthesizes a standard method set. Forms are the
mechanism Aperio uses in place of parametric collection types
(`Map<K, V>`, `Vec<T>`, etc.). See
[`notes/agent-onboarding/aperio-design-philosophy.md`](../notes/agent-onboarding/aperio-design-philosophy.md)
for the design philosophy and `spec/design-rationale.md` for The
Design's grounding (F.0 form-before-parameter, F.22 capacity).

This document specifies the form annotation system in general
(syntax, contract, verification) and the `@form(vec)` contract
in detail. Subsequent forms (`@form(hashmap)`, `@form(ring_buffer)`)
get their own sections as they're committed.

## Annotation syntax

```
form_annotation = "@form" "(" form_name [ "," form_arg { "," form_arg } ] ")"
form_name       = LOWER_IDENT
form_arg        = IDENT "=" expression
```

A form annotation sits on the line above a `locus` declaration,
like the existing `@projection` annotation:

```aperio
@form(vec)
locus ItemList<T> {
    capacity { heap items of T; }
}
```

- **`form_name`** — the form identifier. Lowercase, single word.
  The v1 form library is fixed (see "v1 form library" below);
  user-defined forms are deferred to a future release.
- **`form_arg`** — keyword arguments specific to the form. Used
  for tuning knobs that don't change storage discipline (e.g.
  `max = 100` for `@form(lru_cache)`).
- **One form per locus.** Composition (`@form(vec) @form(ordered)`)
  is rejected in v1.

Form-specific configuration that *does* change storage
discipline goes on the capacity slot, not in the annotation
arguments — see "`indexed_by` and slot clauses" below.

## Form contract

Each form specifies three things the compiler verifies and
implements:

1. **Required capacity shape.** What slots the locus must
   declare, of what kinds, holding what cell types. Verified at
   typecheck.
2. **Synthesized method set.** Names, parameter types, return
   types of methods the form provides. Injected at typecheck so
   call sites resolve normally.
3. **Lowering strategy.** What C-runtime substrate the compiler
   emits in place of the literal F.22 pool / heap lowering.

If the locus's shape doesn't match the form's required capacity,
the compiler emits a focused diagnostic and rejects the program.
Example:

```
error[FORM-SHAPE]: @form(vec) requires exactly one `heap` slot;
                   found `pool entries of CmdEntry` instead.
   --> registry.ap:3:1
    |
  3 | @form(vec)
    | ^^^^^^^^^^
  4 | locus Registry { capacity { pool entries of CmdEntry; } }
    |                              ----------------------------
    |                              expected `heap items of T`
```

## Synthesized methods

The form *synthesizes* its standard method set. The user does
not declare them; call sites resolve as if they were declared.

```aperio
@form(vec)
locus ItemList<T> {
    capacity { heap items of T; }
    // push, get, pop, len, is_empty come from @form(vec).
}

fn main() {
    let l = ItemListL_Int { };
    l.push(42);
    let head = l.get(0) or raise;
    println(head);  // 42
}
```

**The user CAN add additional methods** on top of the
synthesized standard set. Naming a user method that collides
with a synthesized method (e.g. user writes their own `push`)
is rejected at v1 — override is deferred to v2.

## `indexed_by` and slot clauses

Form configuration splits between *slot clauses* and
*annotation arguments*. The dividing line:

- **Slot clause** — if the configuration changes how cells are
  laid out or accessed. A storage-discipline concern.
- **Annotation argument** — if the configuration is a policy /
  tuning knob the form's runtime consults; the underlying
  storage shape is the same regardless.

```aperio
// Storage discipline — slot clause.
@form(hashmap)
locus CmdRegistry {
    capacity { pool entries of CmdEntry indexed_by name; }
    //                                   ^^^^^^^^^^^^^^^
    //                                   slot clause
}

// Policy / tuning — annotation argument.
@form(lru_cache, max = 100, ttl = 60s)
locus SessionCache {
    capacity { pool sessions of SessionEntry indexed_by id; }
}
```

`indexed_by` is a slot clause because indexing IS a storage
commitment — it changes the pool's layout and access path.

## Default lowering (no form annotation)

A locus without `@form(...)` gets the **literal F.22 default
lowering**: pool slots become `lotus_pool_t*` chunked free-list;
heap slots become `lotus_heap_t*` doubling buffer. The user's
own methods run as written; no synthesis, no shape verification
beyond the normal capacity-slot machinery.

The form annotation is the user's opt-in to a specific efficient
lowering. Without it, you get the predictable F.22 default.

## Form-annotated loci as application-layer storage substrate

A `@form(...)` locus occupies a different position in The
Design's taxonomy than a user-declared locus, and the
distinction is load-bearing for the two-channel failure rule
(`spec/semantics.md` § "Fallible call semantics" § "Where
each channel lives"):

- **User-declared loci** are substrate-facing — they
  participate in the locus tower's lifecycle (bus
  subscriptions, modes, contract reads, lifecycle methods).
  Their methods communicate failure structurally via
  closure assertions + `on_failure` routing.
- **`@form(...)` loci** are application-layer storage
  substrate — they realize a substrate-honest *container*
  shape that application code uses to hold data. Their
  synthesized methods (`@form(vec).get`, `@form(vec).pop`,
  future `@form(...)` accessors) operate per-access and
  may be declared `fallible(E)`, addressing failure at the
  immediate caller's `or` clause.

This is why the synthesized `@form(vec)` `get` / `pop`
methods carry `fallible(IndexError)` while user-declared
locus methods cannot. The `@form(...)` annotation is the
declaration-site marker that "this locus is application-
layer storage substrate, not a substrate-structural
participant." The synthesized method surface gets the
application-layer failure channel; the underlying form-vec
locus still respects every other substrate invariant
(arena ownership, dissolve cascade, capacity slot
discipline).

## Perspectives and forms

> **Perspectives reflect on structure, not on lowering.**

The form annotation changes how the compiler lays out memory
and synthesizes methods. It does not change:

- The locus's name or place in the tower.
- The set of fields declared in `params`.
- The capacity slot declarations.
- The `closure` / `on_failure` / `bus` blocks.

Perspectives that reflect on a form-lowered locus see the
*structural* view: the capacity slots, the params, the method
signatures (synthesized or user-written, treated uniformly).
They do not see the underlying C struct layout.

## Performance commitment

> **A form-lowered locus must run within 10% of a hand-written
> equivalent in idiomatic C.**

The 10% gate is verified before any new form is added to the
library. `@form(vec)` is the first form to ship and is the
canonical benchmark target (see "Bench protocol" under the
`@form(vec)` section below).

If a form fails the gate, the lowering is redesigned before
shipping more forms. The point of the form machinery is not to
be clever — it's to be roughly as fast as the C the user would
have written by hand, with all the locus tower's structural
benefits on top.

---

# `@form(vec)`

A contiguous, growable buffer of `T`. The Aperio analogue of
`Vec<T>` / `std::vector<T>` / Go slices. First form committed
for v1; canonical benchmark target for the 10% perf gate.

## Required capacity shape

The locus MUST declare exactly one `heap` slot. Its cell type
becomes the vec's element type `T`.

```aperio
@form(vec)
locus ItemList<T> {
    capacity { heap items of T; }
}
```

Rules verified at typecheck:

- Exactly one slot. Zero slots, more than one slot, or any
  `pool` slot is rejected.
- The slot MUST be a `heap` slot. (`pool` is the unordered free-
  list shape; `vec` is the contiguous shape — they're different
  storage disciplines, so a `pool` declaration with `@form(vec)`
  is a contradiction.)
- The slot name is user-chosen and is not part of the contract.
  The compiler finds the form's heap slot by *position*, not by
  name. Idiomatic spellings: `items`, `entries`, `bytes`, `xs`.

The cell type `T` may be:

- A primitive (`Int`, `Float`, `Bool`, `Decimal`, `Time`,
  `Duration`, `String`, `Bytes`).
- A user-defined `type` (struct or enum).
- A generic parameter (`heap items of T` inside a generic locus
  `ItemList<T>`); monomorphization (m63) produces a concrete
  `@form(vec)` instance per binding.

The cell type MAY NOT be a locus reference — vecs hold values,
not loci. If you want a vec of child loci, use the F.22 `pool`
projection-class machinery instead; that's the structural shape
for parent-owns-children.

## Synthesized methods

```
fn push(x: T) -> ()                          # infallible
fn get(i: Int) -> T fallible(IndexError)
fn pop() -> T fallible(IndexError)
fn len() -> Int                              # infallible
fn is_empty() -> Bool                        # infallible
```

The fallible methods return the locus-defined `IndexError`
payload type:

```aperio
type IndexError {
    kind: String;   # "out_of_bounds" or "empty"
    index: Int;     # the requested index (0 for empty-pop)
    len: Int;       # the vec's len at fail time
}
```

`IndexError` is defined in the synthesized form preamble; the
user does not declare it. The same type is shared across all
`@form(vec)` instantiations (it's a flat record, not parametric
over T).

### `push`

```
fn push(x: T) -> ()
```

Appends `x` after the last element. Amortized O(1). The
synthesized lowering grows the underlying buffer by doubling
when capacity is exhausted; the realloc cost amortizes across
N appends to O(N) total.

`push` is **infallible**. OOM during the doubling realloc is a
substrate-level concern: the C runtime traps malloc failure and
re-raises as a closure violation, not as a `fallible(...)`
return on `push`. From the language surface, `push` never
errors. (See `spec/runtime.md` for the OOM trap convention.)

### `get`

```
fn get(i: Int) -> T fallible(IndexError)
```

Returns the element at index `i` (0-based). If `i < 0` or
`i >= len()`, fails with `IndexError { kind: "out_of_bounds",
index: i, len: self.len() }`.

Idiomatic call sites:

```aperio
let head = vec.get(0) or raise;             # bubble on empty
let first = vec.get(0) or default_value;    # substitute
let nth = vec.get(i) or handle_oob(err);    # custom handler
```

### `pop`

```
fn pop() -> T fallible(IndexError)
```

Removes and returns the last element. If `len() == 0`, fails
with `IndexError { kind: "empty", index: 0, len: 0 }`.

`pop` does not free the underlying buffer — capacity does not
shrink. Buffer release happens at locus dissolution.

### `len` and `is_empty`

```
fn len() -> Int
fn is_empty() -> Bool
```

`len()` returns the number of elements currently in the vec.
`is_empty()` is sugar for `len() == 0`. Both are infallible and
O(1).

## Lowering strategy

`@form(vec)` lowers the heap slot to a three-field C struct:

```c
typedef struct {
    size_t cap;     // allocated capacity (elements)
    size_t len;     // number of valid elements
    T*     buf;     // contiguous element array
} lotus_vec_<T>_t;
```

- Initial capacity: `0` (no allocation at birth). First `push`
  allocates the initial buffer.
- Initial buffer size on first push: `4` elements. Chosen as a
  small constant that avoids the malloc-per-element shape
  without over-allocating for short-lived vecs.
- Growth policy: double `cap` on overflow. New buffer is
  malloc'd; old elements are `memcpy`'d; old buffer is freed.
- Shrink policy: none. Capacity is monotonic in v1. (A
  `shrink_to_fit` method may be added later if a workload
  surfaces the need.)

Element storage is by-value: a `@form(vec)` of `Int` is a
contiguous `int64_t[]`; a `@form(vec)` of `type Pair { x: Int;
y: Int; }` is a contiguous array of `{int64_t, int64_t}`
records. No per-element heap allocation, no per-element header.

For elements of pointer-shaped types (`String`, `Bytes`), the
vec stores the pointer by value; the pointed-to bytes live in
whatever arena they were allocated from. Dissolution of the vec
frees the buffer but does not free the pointed-to bytes — those
follow their owning arena's lifetime per the standard F.22
contract.

## Arena ownership

`@form(vec)` is **not** a separate arena-allocated structure.
The three-field `lotus_vec_*` struct lives inline in the locus's
struct layout, the same way the literal `heap items of T`
declaration would. The growable buffer (the `buf` field) is
malloc'd from the *system allocator*, not from the locus's
arena — this is the existing F.22 heap-slot contract, unchanged.

Dissolution: when the locus arena is destroyed, the vec's
`buf` is freed via the F.22 dissolve cascade (the synthesized
destructor emits `free(self.items.buf)` for each formed heap
slot).

## Interaction with the locus tower

A `@form(vec)` locus is a locus in every other respect. It can:

- Have `params { ... }` with defaults.
- Have `birth`, `run`, `drain`, `dissolve` lifecycle bodies.
- Declare `closure { ... }` invariants.
- Route failures via `on_failure(child, err)`.
- Participate in a `bus`.
- Be projected by `perspective` declarations.

These are orthogonal to the form annotation. The form
*replaces* the literal F.22 heap-slot lowering; it does not
replace any other locus mechanic.

## Bench protocol (FORM-3 gate)

`@form(vec)` is the canonical benchmark target. Before any
additional form ships, `@form(vec)` must demonstrate:

1. **Microbench.** 1M `push` followed by 1M random-index `get`
   on a `@form(vec)` of `Int`, compared against an equivalent
   hand-written C program using `malloc` + doubling realloc and
   raw `int64_t[]` indexing. Wall-clock and peak RSS. The
   `@form(vec)` lowering must come within 10% of the C baseline
   on both metrics.
2. **App bench.** A representative app (ferryman is the
   tentative candidate, given its parsing-heavy workload) is
   rewritten to use `@form(vec)` where it currently does
   explicit F.22 pool walks. Wall-clock and RSS compared
   before / after, with the form-lowered version targeted to be
   no worse than the F.22 baseline.

Both benches live under `bench/forms/vec/` (path to be created
when FORM-3 starts). The microbench harness is a fresh binary
target; the app bench reuses ferryman's existing harness.

If either bench fails the 10% gate, the lowering is redesigned
before further forms are added to the library.

## Anti-patterns

### Hand-rolling the contract on a form-annotated locus

```aperio
// WRONG — @form(vec) synthesizes push; user declaration
// collides with the synthesized name.
@form(vec)
locus ItemList<T> {
    capacity { heap items of T; }
    fn push(x: T) -> () { /* ... */ }  // rejected
}
```

The compiler rejects this at typecheck with `error[FORM-COLLIDE]:
@form(vec) synthesizes `push`; user declaration shadows the
synthesized method (override is deferred to v2).`

### Ignoring the fallible return

```aperio
// WRONG — `get` returns fallible(IndexError); the bare let
// binding drops the error.
let v = vec.get(i);  // compile error: error not addressed
```

```aperio
// RIGHT — address the error with one of the three motions.
let v = vec.get(i) or raise;
```

### Treating the form annotation as syntactic sugar

```aperio
// WRONG — assumes @form(vec) is "just like" hand-writing the
// methods over a literal F.22 heap. The lowering is different;
// the storage layout is different; the perf characteristics
// are different.
```

The form is a *contract*, not sugar. It commits to a specific
lowering and performance shape. Code that depends on
implementation details of the literal F.22 heap-slot lowering
(e.g. memory addresses of individual cells across pushes) will
not behave the same under `@form(vec)`.

## Open questions deferred to FORM-2 / FORM-3

These are spec-level questions the FORM-2 implementation work
will answer concretely. They don't block FORM-1 because the
contract above is independent of them.

1. **Iteration surface.** A `for x in vec.items { ... }` form
   is natural, but the loop construct's lowering depends on
   what the existing `for` over F.22 heap slots does. Deferred
   until the implementation pass.
2. **Bulk operations.** `extend(other: @form(vec))`, `clear()`,
   `truncate(n: Int)`. Useful but not foundational. Add after
   the core five methods land.
3. **Mutation in place.** `set(i: Int, x: T) -> () fallible(IndexError)`.
   Mirrors `get` for write. Likely added in FORM-2; left out of
   the v1 core only because the bench workloads don't require
   it.

---

# `@form(hashmap)`

A keyed associative store: each entry is a struct value `S` that
carries its own key as one of its fields. The Aperio analogue of
`Map<K, V>` / `std::unordered_map` / Go `map[K]V` — but
*intrusive*: the value type S carries the key inside it rather
than the map storing separate (K, V) pairs. Shipped as the second
form in v1, following `@form(vec)`.

## Required capacity shape

The locus MUST declare exactly one `pool` slot, with an
`indexed_by <field>` clause naming a field of the cell type to
serve as the key:

```aperio
type CmdEntry {
    name: String;
    handler: Int;
}
@form(hashmap)
locus CmdRegistry {
    capacity { pool entries of CmdEntry indexed_by name; }
}
```

Rules verified at typecheck:

- Exactly one slot. Zero slots, more than one slot, or a `heap`
  slot is rejected.
- The slot MUST be `pool`. (Hashmap recycles entry cells as
  inserts / removes flow — the `pool` discipline. `heap` is the
  growable-contiguous shape covered by `@form(vec)`.)
- The slot MUST declare `indexed_by <field>`. The named field
  must exist on the cell type.
- The cell type MUST be a user-declared `type` struct.
  Primitives, enums, type aliases, locus references, and
  qualified paths are rejected — the substrate needs the cell's
  field layout to GEP the key out at insert time, which only
  resolves cleanly for struct cells.
- The slot name is user-chosen and is not part of the contract.
  The compiler finds the form's pool slot by *position*, not by
  name. Idiomatic spellings: `entries`, `bindings`, `routes`.
- `as_parent_for` on the slot is rejected — `@form(hashmap)`
  owns its slot's allocator, so the borrow mechanic from
  v1.x-4b doesn't compose.
- `@form(hashmap, ...)` with annotation arguments is rejected:
  there are no tuning knobs at v1.

The key type `K` is derived from the resolved type of the
indexed-by field. At v1, K must be `Int` or `String`. Other
field types parse and synthesize methods but reject at codegen
with a focused diagnostic (the runtime ABI's `key_type_tag`
only enumerates these two).

## Synthesized methods

```
fn get(key: K) -> S fallible(KeyError)
fn set(value: S) -> ()                       # infallible
fn has(key: K) -> Bool                       # infallible
fn remove(key: K) -> () fallible(KeyError)
fn len() -> Int                              # infallible
fn is_empty() -> Bool                        # infallible
```

The fallible methods return the synthesized `KeyError` payload:

```aperio
type KeyError {
    kind: String;   # "missing_key" — only kind at v1
}
```

`KeyError` is injected into the bundle scope by the form
machinery alongside `IndexError`. The same type is shared across
all `@form(hashmap)` instantiations.

The key is not carried on the error because K varies per
hashmap. Users who want key context construct it through the
substitute motion (`or fallback(err)`), where `err: KeyError`
is in scope and any of the call's local bindings — including
the key arg — are available.

### `set`

```
fn set(value: S) -> ()
```

Inserts or replaces. `set(v)` GEPs the indexed-by field from
`v` to derive the key, then writes the whole struct at the
hashed slot. If a previous entry shared the key, it is
overwritten (`set` is unconditional — no error on duplicate).

`set` is **infallible**. OOM during the doubling realloc is a
substrate-level concern routed through the closure-violation
channel, not a `fallible(...)` return. Same shape as
`@form(vec)`'s `push`.

### `get`

```
fn get(key: K) -> S fallible(KeyError)
```

Returns the entry whose indexed-by field equals `key`. If no
such entry exists, fails with `KeyError { kind: "missing_key" }`.

```aperio
let entry = registry.get(name) or raise;
let entry = registry.get(name) or default;
let entry = registry.get(name) or fallback(err);
```

### `has`

```
fn has(key: K) -> Bool
```

`true` iff an entry with this key is present. Equivalent to
"`get(key)` would succeed" but cheaper — no value copy.

### `remove`

```
fn remove(key: K) -> () fallible(KeyError)
```

Removes the entry whose indexed-by field equals `key`. If no
such entry exists, fails with `KeyError { kind: "missing_key" }`.
Idiomatic call shape:

```aperio
registry.remove(name) or raise;        # bubble on missing
registry.remove(name) or ignore(err);  # swallow via Unit-returning handler
```

Aperio doesn't surface `()` as a literal expression at v1, so
swallowing the error requires a Unit-returning handler call (or
guarding with `has` first):

```aperio
fn ignore(_e: KeyError) { }
// later:
registry.remove(name) or ignore(err);
```

`remove` does not shrink the underlying buffer; capacity does
not decrease. Buffer release happens at locus dissolution.

### `len` and `is_empty`

```
fn len() -> Int
fn is_empty() -> Bool
```

`len()` returns the entry count; `is_empty()` is sugar for
`len() == 0`. Both infallible, O(1).

## Lowering strategy

`@form(hashmap)` lowers the pool slot to an inline six-field C
struct holding open-addressing hashtable state:

```c
typedef struct {
    size_t cap;          // power-of-two slot count
    size_t len;          // live entry count
    size_t key_size;     // sizeof(K), set at init
    size_t value_size;   // sizeof(S), set at init
    int    key_type_tag; // 0 = Int, 1 = String
    char  *slots;        // cap * (1 + key_size + value_size) bytes
} lotus_hashmap_t;
```

Each slot is `1 + key_size + value_size` bytes laid out as
`[occupied: u8][key: K][value: S]`. `occupied = 0` means empty;
the runtime uses **backward-shift deletion** (no tombstones) so
probes terminate as soon as an empty slot is seen.

- **Initial cap:** 8 slots, allocated at locus birth via
  `lotus_hashmap_init`. Power of two so hash → index folds to
  a single `& mask`.
- **Growth policy:** double `cap` when `(len + 1) > 0.7 * cap`.
  Rehash every live entry through the normal `set` path (the
  probe sequence changes with the new mask).
- **Shrink policy:** none. Capacity is monotonic in v1.
- **Hash functions:** 64-bit Knuth multiplicative for Int keys
  (`k * 0x9E3779B97F4A7C15`), FNV-1a over the bytes for String
  keys.
- **Probing:** linear with `& mask`. Backward-shift deletion
  walks the cluster forward, shifting any entry whose natural
  position is "before" the freed slot. Cluster boundary is the
  first empty slot.

### Key extraction at the codegen surface

At each `set(value: S)` call site, codegen GEPs the indexed-by
field offset on the value alloca to produce a pointer to the
key, then passes `(slot_ptr, key_ptr, value_ptr)` to
`lotus_hashmap_set`. The runtime memcpys `key_size` bytes from
`key_ptr` into the slot's key region and `value_size` bytes
from `value_ptr` into the value region.

At `get`, `has`, `remove` sites, codegen lowers the key arg
into an alloca matching `key_size` and passes its address.

## Arena ownership

`@form(hashmap)` is **not** a separate arena-allocated structure.
The `lotus_hashmap_t` struct lives inline in the locus's struct
layout, the same way the literal F.22 pool-slot declaration
would. The `slots` buffer is malloc'd from the *system
allocator*, not from the locus's arena — matching the existing
F.22 slot contract.

Dissolution: when the locus arena is destroyed, the hashmap's
`slots` buffer is freed via `lotus_hashmap_destroy` in the F.22
dissolve cascade.

For elements of pointer-shaped types (`String`, `Bytes`) in the
cell struct, the hashmap stores the pointer by value; the
pointed-to bytes live in whatever arena they were allocated
from. Hashmap dissolution frees the slots buffer but does not
free the pointed-to bytes — those follow their owning arena's
lifetime per the standard F.22 contract.

## Complexity

| Operation | Expected | Worst case |
|---|---|---|
| `set` (no resize) | O(1) | O(N) on probe cluster |
| `set` (with resize) | O(N) amortized over inserts | O(N) per resize |
| `get` / `has` | O(1) expected | O(N) on probe cluster |
| `remove` | O(1) expected | O(N) on shift |
| `len` / `is_empty` | O(1) | O(1) |

Load factor stays ≤ 0.7 by construction. Hash quality for Int
keys (Knuth multiplicative) handles dense sequences such as
consecutive IDs without all colliding on slot 0.

## Interaction with the locus tower

A `@form(hashmap)` locus is a locus in every other respect — it
can have `params`, lifecycle bodies (`birth` / `run` / `drain` /
`dissolve`), `closure` invariants, `on_failure` routing, bus
membership, and projection by `perspective` declarations. The
form annotation *replaces* the literal F.22 pool-slot lowering
and synthesizes the six methods; it does not replace any other
locus mechanic.

## Bench protocol (future FORM-N gate)

Once `@form(vec)`'s 10% bench gate (FORM-3) is satisfied,
`@form(hashmap)` gets its own bench under a parallel FORM-N
milestone:

1. **Microbench.** 1M `set` followed by 1M `get` (each with
   uniformly-random keys drawn from a population larger than
   the load-factor expansion), compared against an equivalent
   hand-written C program using `malloc` + linear probing
   tables of the same shape. The `@form(hashmap)` lowering
   must come within 10% of the C baseline on wall-clock and
   peak RSS.
2. **App bench.** A representative app rewritten to use
   `@form(hashmap)` where it currently does explicit registry
   walks; before / after comparison.

Both benches will live under `bench/forms/hashmap/`. Not gated
on FORM-4 shipping; ships as a separate milestone after the
core surface stabilizes.

## Anti-patterns

### Treating `set` as keyed insert

```aperio
// WRONG — set takes the whole value, not (key, value).
registry.set("foo", entry);   // type error: too many args
```

```aperio
// RIGHT — value carries its key as a field; substrate extracts.
registry.set(CmdEntry { name: "foo", handler: 1 });
```

The intrusive shape means the type system catches this for you
(`set` is synthesized with the single-arg signature `set(value:
S) -> ()`), but the conceptual reflex from `HashMap<K, V>`
shaped languages is worth flagging.

### Ignoring the fallible return

```aperio
// WRONG — get and remove return fallible(KeyError).
let v = registry.get(name);          // compile error: error not addressed
registry.remove(name);               // compile error: error not addressed
```

```aperio
// RIGHT — address the error via one of the three motions.
let v = registry.get(name) or raise;
registry.remove(name) or ();
```

### Mutating the indexed-by field after `set`

The intrusive shape means the key is the field. If user code
keeps a reference to the value and mutates the indexed-by
field, the hashmap's invariant breaks (the cell sits in the
slot keyed by its *old* key, but `get` now looks up by its
*new* key). The v1 surface doesn't expose stored cells by
reference, so this isn't reachable from user code today.
Future iteration APIs that surface entry references will need
to gate against indexed-by-field mutation.

## Open questions deferred to a future milestone

These are spec-level questions that don't block FORM-4 because
the core surface above is independent of them.

1. **Iteration surface.** `for entry in registry { ... }` is
   natural but the loop construct's lowering depends on what
   the existing `for` over capacity slots does — and a hashmap
   iteration that visits each occupied slot once needs cluster-
   aware traversal. Deferred.
2. **Bulk operations.** `clear()`, `extend(other)`,
   `take(key) -> S fallible(KeyError)` (get + remove fused).
   Useful but not foundational. Add after a workload demands.
3. **Additional key types.** `Bytes`, custom structs with a
   hashable derivation, enum tags. Each adds a `key_type_tag`
   to the runtime ABI. Workload-driven.
4. **Capacity hints.** `@form(hashmap, cap = 64)` is rejected
   in v1; no tuning knobs. Add when a workload demonstrates
   the 0 → 8 → 16 → ... grow cascade is costing measurable
   time.
5. **Set type.** A `@form(set)` would be a hashmap-without-
   value variant (the cell IS the key). Not part of FORM-4;
   revisit if a workload needs it.

# `@form(ring_buffer)` — pending future milestone

FORM-4 shipped `@form(hashmap)` only; `@form(ring_buffer)` waits
for a concrete driver workload that the fixed-size pop-front /
push-back surface is the right shape for. Spec to be written
when that milestone starts. Surface preview:

```aperio
@form(ring_buffer, cap = 64)
locus RecentCmds {
    capacity { pool history of CmdEntry; }
}
```

Synthesized methods:

```
fn push(x: T) -> Bool          # returns false when full
fn pop() -> T fallible(EmptyError)
fn len() -> Int
fn is_full() -> Bool
```

Lowering: fixed-size array of size `cap` + head/tail indices,
no malloc after birth.
