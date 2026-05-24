# F.32 — Cache-aware locus substrate: delivery plan

**Spec anchor:** `spec/design-rationale.md` § F.32 "Locus
working set as a cache-budget primitive (sketch)".

**Motivation.** The downstream workload (multi-venue
market-data gateway + HFT-adjacent strategy code) demands
tail-latency control. Hale's locus model already exposes
the partitioning a cache hierarchy wants — region semantics
+ lifetime + thread isolation + declared bounds + vertical-
only flow. The structural foundation is there; the active
analysis isn't. This plan turns the structural advantage
into measurable wins.

This document survives a repository / organization rename;
all references to "hale" / "hale" should be read
against the new names once the rename lands. Code paths
below use the directory layout as of `3869ffa` on `main`;
substring-rename should suffice.

---

## Scope summary

Five deliverables, increasing in cost, decreasing in
expected impact-per-unit-effort. Each is independently
shippable — earlier deliverables can land without later
ones, and the structural foundation post-deliverable-1 is
enough to attack the rest on demand.

| ID | Theme | Effort | HFT impact |
|---|---|---|---|
| **F.32-1**  | False-sharing padding on cross-pool `@form` cells | small | high |
| **F.32-1b** | Locus struct field reordering by access frequency | small-medium | medium |
| **F.32-2**  | Compile-time working-set budget per locus | medium | medium (engineering discipline; CI gate) |
| **F.32-3**  | Per-pool arena chunking sized to cache slice | medium | low-medium (multi-pool scale-up) |
| **F.32-4**  | HFT extras: huge pages, prefetch hints, mlock | small-medium | medium-high |

Recommended order: **1 → 4 (prefetch sub-task) → 1b → 2 →
3 → 4 (huge pages + mlock)**. Rationale: false-sharing fix
is the immediate ask; prefetch hints in bus dispatch are
nearly-free given the substrate already knows the
producer/consumer cell relationship; field reordering
extends the same access-pattern analysis; budgets +
chunking are scale-up concerns; huge pages + mlock are
deployment polish.

---

## F.32-1 — False-sharing padding on cross-pool `@form` cells

**The problem.** A `@form(hashmap)` locus declared on a
non-`main` cooperative pool is reachable cross-pool via the
@form-cross-pool exemption (`spec/types.md` § Single-
threaded-method invariant → "Interaction with `@form(...)`
loci", shipped 2026-05-24 / `3ec6391`). Two cores writing
adjacent cells that share a 64-byte cache line generate
MESI ping-pong even though each producer logically owns
its own cell. The Prometheus-registry pattern (one Counter
per metric, multiple producer pools incrementing, one
consumer pool rendering) hits this directly.

**The fix.** At codegen time, detect `@form(hashmap)` /
`@form(vec)` loci that are reachable from main on a
non-`main` pool. For those, pad cell stride up to the next
`LOTUS_CACHE_LINE` multiple (64B default). Other form loci
keep the current packed stride.

**Concrete substrate touch-points.**

1. **Add `LOTUS_CACHE_LINE` constant.** `runtime/lotus_arena.c`,
   alongside `LOTUS_HASHMAP_INITIAL_CAP`. Default 64. A
   `--cache-line-size=N` CLI flag overrides at build time
   for non-x86_64 targets (ARM big.LITTLE has variable line
   sizes; M-series Apple is 128B effective).

2. **Detection.** A new pass in
   `crates/hale-codegen/src/codegen.rs` after
   `collect_main_placement` (which already populates
   `main_cooperative_pools` + `pinned_locus_types`).
   Produces a `BTreeSet<String>` of locus type names that
   are (a) `@form(hashmap)` or `@form(vec)`, AND (b) appear
   as a non-`main` placed locus type OR contain a pool ptr
   field reachable from a non-`main` pool subscriber. The
   transitive reachability walk is bounded by the F.22
   capacity-cell graph — already known statically.

3. **Padding emission.** In
   `lower_locus_instantiation` / `declare_locus_struct`,
   when the locus is in the cache-padded set, compute the
   padded cell size: `padded_stride = round_up(packed_stride,
   LOTUS_CACHE_LINE)`. Pass that stride into
   `lotus_hashmap_init` as `value_size + (padded_stride -
   packed_stride)` worth of trailing padding bytes. The
   hashmap C-runtime already takes `key_size` + `value_size`
   independently; this is a one-line per-call adjustment.

4. **Opt-out annotation.** `@form(hashmap, packed)` or
   `@form(hashmap, no_pad)` annotation arg. Parser already
   admits `FormAnnotation { name, args: Vec<...> }`; lex one
   more kwarg. Typecheck flags conflicting args
   (`packed` + cross-pool reachable = warning: padding
   suppressed despite cross-pool dispatch). Honor the opt-
   out unconditionally — the author knows their memory
   budget.

5. **Tests.**
   - `crates/hale-codegen/tests/form_cache_padding.rs`:
     compiles a 4-cell hashmap on a non-main pool, asserts
     `lotus_hashmap_entry_size(&map) >= 64` post-padding;
     same shape with `packed` opt-out asserts packed stride.
   - `crates/hale-codegen/tests/form_cache_padding_perf.rs`
     (gated `#[ignore]`, run with `--ignored`): two producer
     threads pinned to different cores hammer adjacent cells
     for N iterations; consumer thread reads. Measures
     `clock_gettime(CLOCK_THREAD_CPUTIME_ID)` delta with vs.
     without padding (via `@form(hashmap, packed)`). Asserts
     padded version >= 1.5x faster — conservative; real
     speedup on cores with shared L2 is 3-5x.

6. **Spec / docs.**
   - `spec/design-rationale.md` § F.32: promote sketch → v1
     section once shipped. Note the cell-stride change in
     the ABI table.
   - `spec/forms.md` (if it exists; else `spec/stdlib.md` §
     "@form annotations"): document the `packed` kwarg.
   - `docs/src/how-tos/threading.md`: note that cross-pool
     `@form(hashmap)` cells are automatically padded;
     `packed` opt-out is for non-cross-pool dense storage.

**Acceptance.** The perf fixture shows measurable speedup
(target: ≥2x latency reduction on the producer/consumer
hot loop, measured on a 2+ core machine with siblings on
the same L2). False-sharing pmu counters (`perf stat -e
mem_load_l3_hit_xsnp_hitm`) drop to ~zero on the padded
version.

**Out of scope.**
- Padding for non-cross-pool form loci (pure intra-pool
  workloads). Keep the dense layout.
- Padding for `@form(ring_buffer)` — cells already produce/
  consume in FIFO order, not parallel; line sharing across
  cells doesn't cause MESI traffic.
- Manual cache-line size at the cell-type level (i.e., a
  `@cache_line(128)` annotation on the cell type). Defer.

**Estimated effort.** 1 focused session. ~300 LOC across
codegen + runtime + tests.

---

## F.32-4-prefetch — Bus-dispatch prefetch hint

(Pulled forward from F.32-4 because it pairs naturally
with -1: same hot path, same detection logic.)

**The opportunity.** Cross-pool bus dispatch already memcpys
the payload into the destination pool's queue cell
(`lotus_coop_pool_post` → ring buffer slot). The receiver
pool's worker drains by reading that exact cell first. If
we emit `__builtin_prefetch(slot, 1, 3)` immediately after
the memcpy in the producer, the destination cache line is
already inbound on the receiver's L1 by the time the
receiver's drain wakes.

**Implementation.** One-line addition in
`lotus_coop_pool_post` after the slot fill:
`__builtin_prefetch(slot, 1, 3);` (write-intent, high
temporal locality). Same for `lotus_mailbox_post`. Zero
cost on the producer side (single instruction, no stall);
~10-50ns saved on the receiver side per cell.

Same change in `lotus_bus_queue_enqueue` for the main-pool
cooperative path, though the win is smaller there (same-
core drains tend to be cache-warm already).

**Tests.** Hard to assert programmatically without HW perf
counters; ship behind a build flag (`--enable-prefetch`,
default on) and rely on the perf fixture from F.32-1 to
detect regressions.

**Effort.** <1 hour. ~10 LOC.

---

## F.32-1b — Locus struct field reordering by access frequency

**The opportunity.** A locus's methods are statically
visible. The compiler can compute, for each field, how
many method bodies touch it (read or write). Reordering
fields so that high-access fields land on the first cache
line of the struct (after the synthetic header fields)
keeps method-body hot reads on a single line; cold fields
(set once at birth, read at dissolve) migrate to later
lines.

**Caveats.**
- Synthetic fields (`__arena`, `__quarantined`,
  `__parent_self`, etc.) have fixed positions for ABI
  reasons. Reordering applies only to user-declared
  `params` fields.
- Default field order is source-declaration order, which
  many existing programs implicitly rely on (struct
  literal field-name pairs are explicit, but
  intermediate code might assume order via direct GEPs).
  Need to verify codegen doesn't bake offsets into
  intermediate state — the locus's `info.fields:
  BTreeMap<String, (u32, CodegenTy)>` already keys by
  name, so this looks safe.
- Reordering must be deterministic across builds for
  cross-binary bus compatibility. Sort by (access_count
  desc, declaration_order asc).

**Implementation.** New pass in
`declare_locus_struct` after the synthetic field
positions are fixed: walk method bodies once with a
field-access counter, sort user fields by count, emit
the LLVM struct with the new order. Update `info.fields`
indices accordingly.

**Annotation override.** `@layout(declaration_order)` on a
locus disables reordering for cases where the author
needs ABI stability (e.g., cross-binary bus payload
types where the wire format is field-order-dependent —
though those should be `type` decls not loci, and the
wire format is per-field-serialized not memcpy-shaped, so
this is theoretical).

**Tests.**
- Verify reordering happens via IR dump
  (`LOTUS_DUMP_IR=1`) — assert field positions in the
  emitted struct match the by-access-frequency order.
- Regression: existing programs continue to work
  (struct literal field-name pairs already abstract over
  position).

**Effort.** 1 small session. ~150 LOC.

---

## F.32-2 — Compile-time working-set budget per locus

**The deliverable.** A build-time analysis that computes
each locus's projected working set and compares against
a target cache budget. Out-of-budget loci produce a
warning naming the tower depth at which the budget
overflows.

**Working-set formula.**

```
working_set(L) =
    sizeof(L's struct)
  + sum(c in L's capacity slots) cap(c) * cell_stride(c)
  + sum(child in L's params) working_set(child)
                                 if child is locus-typed
  + L's per-method scratch high-water mark (heuristic;
    bound by largest known transient allocation)
```

Capacity for chunked / recognition projections comes from
F.22's compile-time bounds. `@form(hashmap)` capacities
come from `capacity { pool entries of T indexed_by k; }`
declarations — these accept an optional `cap = N` kwarg
(parser surface exists; lift to required for budget
analysis or default to a large sentinel).

**Surface.**

```hale
@locality(L1)   // working-set MUST fit in L1
@locality(L2)   // ... L2
@locality(L3)
@locality(any)  // explicit "no budget" (default if unannotated)
locus HotPath { ... }
```

```sh
hale build . --target-cache=l2     # warn on >L2
hale build . --target-cache=l1 --strict   # error
```

Without `--target-cache`, no analysis runs (zero cost).

**Implementation.**

1. New crate or module:
   `crates/hale-types/src/working_set.rs`. Walks LocusInfo
   recursively, sums bounded sizes, returns
   `WorkingSetEstimate { lo: usize, hi: usize, unbounded:
   bool }`. Tower depth tracked as a string path for the
   diagnostic.
2. Diagnostic site: post-typecheck, pre-codegen pass in
   `hale-cli/src/main.rs`. Emits `warning: locus 'L'
   working set ~38 KB exceeds @locality(L1) ≈ 32 KB; chain:
   App → Mdgw → BookEngine (cells: 4096 × 8 bytes = 32 KB)`.
3. Cache-tier constants: build-time defaulted from
   `/sys/devices/system/cpu/cpu0/cache/index{0,2,3}/size`
   on Linux; fall back to 32K / 512K / 8M.

**Tests.**
- Fixture with three loci, each annotated `@locality(L1)` /
  `(L2)` / `(L3)`. Assert the analyzer flags the L1-budgeted
  one when its cells push it over.
- Fixture with bounded + unbounded loci asserts the
  unbounded-leaf path produces a "cannot compute budget"
  diagnostic rather than a false-pass.

**Effort.** 1-2 sessions. ~500-800 LOC including diagnostics
and tests.

---

## F.32-3 — Per-pool arena chunking sized to cache slice

**The opportunity.** Today the per-locus arena allocator
picks default chunk sizes via its own grow heuristic. On a
cooperative pool with N loci sharing one OS thread, those
N loci collectively compete for that core's L2 slice
(typical 1 MB per core on modern Intel; 12 MB shared L3).
If chunk sizes are large relative to L2-per-core / N, each
locus's chunk evicts the others on rotation through the
pool worker's drain loop.

**The fix.** Per-pool arena chunk sizing based on the
worker's resident-set budget:

```
chunk_size(pool P, locus L) =
    min(default_chunk_size,
        (target_L2_per_core / loci_on(P)) / typical_chunks_per_locus)
```

This is a hint, not a hard cap. Locus methods that need
more allocation still grow chunks beyond the hint; the
hint just makes the FIRST chunk land in the L2-friendly
size band.

**Implementation.**

1. `lotus_arena_create` grows a variant
   `lotus_arena_create_sized(initial_chunk_bytes)`.
2. Codegen at locus instantiation: for loci on a non-
   `main` cooperative pool, emit `_create_sized(hint)`
   instead of `_create()`. Hint computed at codegen
   time from `main_cooperative_pools` + count of loci
   per pool.
3. Default chunk size for main-pool loci unchanged.

**Tests.** Hard to assert without perf measurement; ship as
an opt-in via build flag `--cache-aware-chunking` and rely
on the F.32-1 perf fixture extended to multi-locus-per-pool
shapes.

**Effort.** 1 session. ~200 LOC.

---

## F.32-4 — HFT-specific extras

Three independent sub-deliverables, each scopable
individually.

### F.32-4a — Huge-page-backed arenas for pinned loci

**The opportunity.** Pinned loci with multi-MB working
sets (order books, large hashmap registries) generate TLB
pressure on every cache miss that lands on a new 4K page.
Huge pages (2 MB on x86_64) reduce TLB walks by 512x.

**Implementation.**

1. `lotus_arena_create_hugetlb(initial_bytes)` in
   `runtime/lotus_arena.c`: `mmap(NULL, sz, PROT_READ |
   PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS | MAP_HUGETLB |
   MAP_HUGE_2MB, -1, 0)`. Fallback to `mmap` without
   `HUGETLB` on `ENOMEM` (kernel huge-page pool exhausted).
2. Annotation `@hugepages` on a locus opts in:
   ```hale
   @hugepages
   locus OrderBook { ... }
   ```
3. Threshold check: huge-page arenas only used for arenas
   whose initial chunk is >= 2 MB (anything smaller
   wastes physical memory).
4. Sysctl prereq documented:
   `sysctl -w vm.nr_hugepages=N`. Diagnostic at startup if
   `@hugepages` is declared but hugepages unavailable.

**Effort.** 1 small session. ~100 LOC.

### F.32-4b — Prefetch hints

Already detailed in the F.32-4-prefetch section above.
Ship with F.32-1.

### F.32-4c — `mlockall()` opt-in for latency-critical programs

**The opportunity.** Page faults on a hot-path arena
allocation can cause a multi-millisecond stall (worst case
when swap is enabled and the kernel decides to evict a
page). HFT-grade processes use `mlockall(MCL_CURRENT |
MCL_FUTURE)` to lock all pages.

**Implementation.**

1. Surface on `main locus`:
   ```hale
   main locus App {
       runtime {
           lock_memory: true;
       }
       params { ... }
       placement { ... }
   }
   ```
2. Codegen at main prelude: `mlockall(MCL_CURRENT |
   MCL_FUTURE)` after `lotus_env_init` if the runtime block
   declares it.
3. Sysctl prereq documented:
   `ulimit -l unlimited` or appropriate `RLIMIT_MEMLOCK`.

**Effort.** 1 small session. ~80 LOC.

---

## Sequencing recommendation

**Week 1: F.32-1 + F.32-4-prefetch.** Producer/consumer
false-sharing is the highest-impact, smallest-scope ship.
Prefetch hint comes for free because the detection /
producer-side code path is the same. Land both as one
commit, measure both via the perf fixture, ship.

**Week 2: F.32-1b.** Field reordering. The substrate
already has the method-body walk; this just adds the
access-counter pass + struct-layout reorder.

**Week 3: F.32-2.** Working-set budget. The biggest
deliverable; useful for CI gates on production binaries.

**Week 4: F.32-4a + F.32-4c.** Huge pages + mlockall.
Deployment-grade; useful even before F.32-3.

**Week 5+: F.32-3.** Per-pool chunking. Scale-up concern;
only worth landing once multi-pool deployments are common.

---

## What this plan does NOT cover

- **GPU/accelerator integration.** Out of scope; substrate
  remains CPU-first at v1.
- **Inter-process cache coordination.** Caches don't help
  cross-process; bus-over-unix / shm-ring already pay the
  copy cost at the boundary.
- **NUMA-aware allocation.** Hale doesn't model NUMA
  topology today. A future F.33-shape proposal could
  extend placement to NUMA nodes (`pinned(numa = 0)`).
- **Runtime profile-guided cache adaptation.** Static-only.
  LLVM PGO is orthogonal and stacks on top.
- **`async`/`await`-style work stealing.** F.31 ships M:N
  cooperative pools with one OS thread per pool; work
  stealing within a pool is a v2+ concern (would invalidate
  the per-arena single-thread invariant without further
  work).

---

## Friction items this closes

Once F.32-1 ships:
- Hot-path counter increments across producer pools no
  longer ping-pong the cache line between cores.
- The "Prometheus registry shared across pools" pattern
  becomes free of false-sharing penalty.

Once F.32-1b ships:
- Locus method bodies with hot fields touched on every
  iteration get those fields packed on the first cache
  line, reducing per-iteration L1 miss rate.

Once F.32-2 ships:
- "This tower won't fit in L2 on the chosen target"
  surfaces at build time instead of at perf-measurement
  time.
- CI gates: a regression that pushes a locus over its
  declared `@locality` budget fails the build.

Once F.32-4 ships:
- TLB miss rate on large pinned-locus working sets drops
  by ~512x via huge pages.
- Bus dispatch latency drops by 10-50ns/cell via prefetch.
- Worst-case page-fault stalls eliminated for latency-
  critical programs via mlockall.

---

## Coverage / verification strategy

**Microbenchmarks:** `crates/hale-codegen/tests/cache_*.rs`
fixtures, gated `#[ignore]`, run with `cargo test --release
-- --ignored`. Each fixture pins threads to specific cores
(via `pthread_setaffinity_np` invoked from Rust test code),
runs a hot loop for N iterations, measures `clock_gettime`
or `__rdtsc()` delta. CI doesn't run these (pinning needs
host root or specific kernel config); developers run them
locally before claiming a perf win.

**Functional tests:** every F.32-* deliverable lands with
a functional test asserting the structural behavior
(padding applied, fields reordered, etc.) independent of
perf measurement.

**Real-workload validation:** the downstream gateway daemon
serves as the end-to-end test bed. Pre-F.32-1 latency
profile vs. post-F.32-1 latency profile is the acceptance
gate for the work.

---

## Document survival across rename

This file lives at `notes/f32-cache-aware-delivery-plan.md`.
The rename will swap the org/language name; the substantive
content of this plan is name-independent.

Touchpoints in this file that will need updating post-
rename:
- Path prefixes `crates/hale-*` → new crate names
- `lotus_*` C runtime symbols → new runtime prefix
- `LOTUS_*` C constants → new prefix
- `hale build` / `hale_codegen` references

A `sed` pass over the substring renames in this file
should suffice; the structural plan stays put.

---

## Pickup checklist (for the new home)

1. Confirm the org/language rename is complete in the new
   directory and the rename's commit hash is on `pub`.
2. Update path/symbol references in this file via a single
   sed pass.
3. Branch from `main` as `f32-1-cache-padding`.
4. Implement F.32-1 + F.32-4-prefetch per § above.
5. Land + push for CI; merge ff to main.
6. Move on to F.32-1b. Repeat.

Stable references that DON'T change across rename:
- The F.32 spec section in `spec/design-rationale.md` (the
  section letter survives; the prose may want a wording
  pass for tone consistency with the new branding).
- The `m90 routing` references in this file (m90 is a
  historical milestone tag preserved per CHANGELOG
  conventions).
- The friction-log shape (downstream consumer's friction
  log; not affected by lotus-side rename).
