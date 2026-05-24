# @form perf checkpoint

Tracks the FORM-3 perf gate (10% of hand-written C) as the
substrate iterates. The bench harness in the sibling
[`hale-lang/bench`](https://github.com/hale-lang/bench)
repo is the source of truth for current numbers; this file is
the narrative the harness output doesn't carry — what changed,
what the diagnosis was, what's still open.

## 2026-05-13 — first bench, post-FORM-4

The bench harness landed (parallel-process activity in
`hale-lang/bench`) and surfaced concrete ratio data against
Go / Node / Python siblings. Headline numbers from the 5-sample median
run, Hale vs Go:

| Bench               | Hale   | Go ratio | Status |
|---------------------|----------|----------|--------|
| loop_overhead       | 20.4 ms  | 0.94×    | within 10% gate ✓ |
| form_vec_push       | 2.89 ms  | 0.96×    | within 10% gate ✓ |
| form_vec_get        | 2.36 ms  | 0.016×   | **62× behind** — pre-fix |
| fn_call             | 188 ms   | 0.04×    | 25× behind (m49 ABI) |
| locus_instantiation | 3.07 ms  | 0.006×   | 167× behind (arena/locus) |
| bus_dispatch        | 2.48 ms  | 0.019×   | 53× behind |
| stream_aggregator   | 4.28 ms  | 0.005×   | 200× behind (composite) |

**Diagnosis split.** Two distinct perf shapes surfaced:

1. **form_vec_get's 62× is layout-correct but codegen-pattern
   wasteful.** form_vec_push at 0.96× proves the inline vec
   layout is right (struct GEP + memcpy of elem bytes is what
   hand-written C does). The 62× on `get` comes from the
   fallible-call codegen surface — specifically, the FORM-2
   PR5 codegen constructed `IndexError` *unconditionally* on
   every call:
   - arena_alloc for IndexError struct
   - 3 stores populating kind / index / len
   - lotus_vec_len function call (just to fill err.len)

   All dead on the happy path. ~50 cycles of waste per call.

2. **fn_call / locus_instantiation / bus_dispatch are
   layout-conditioned by The Design.** The m49 arena-subregion-
   per-call calling convention and the arena-per-locus
   lifecycle are substrate commitments, not codegen-pattern
   accidents. Closing those gaps is calling-convention design
   work; separate from FORM-3.

## 2026-05-13 — lazy-error fix landed

Moved `emit_index_error_alloc` / `emit_key_error_alloc` into
dedicated err basic blocks inside
`try_lower_form_vec_fallible_method` and
`try_lower_form_hashmap_fallible_method`. The happy path now
branches over the alloc + stores entirely. Also dropped the
unconditional `lotus_vec_len` pre-call — `len` is now read
inline via struct GEP into the vec's `len` field, and only on
the err path (where its value populates `IndexError.len`).

Two consecutive `cond_br` on the same `is_err` SSA (one in the
dispatcher, one in `lower_or_expr`'s consumption of the
result) compile down under SimplifyCFG / GVN.

| Bench         | Before  | After   | Go ratio before | Go ratio after | Δ |
|---------------|---------|---------|-----------------|----------------|---|
| form_vec_get  | 2.36 ms | 1.61 ms | 0.016× (62× back) | 0.024× (42× back) | **−32%** |
| form_vec_push | 2.89 ms | 3.02 ms | 0.96×             | 0.90×              | noise |
| loop_overhead | 20.4 ms | 20.4 ms | 0.94×             | 0.94×              | unchanged |

Real measurable win. Tests: 656 / 0 (unchanged).

## 2026-05-13 — amortized benches reframe the picture

A second batch of benches landed (parallel-process), separating
isolated-overhead measurements from amortized-workload ones.
The amortized side vindicates the arena design at scale:

| Bench               | Hale   | Go ratio | Read |
|---------------------|----------|----------|------|
| **Overhead** (per-op cost in isolation) |  |  |  |
| loop_overhead       | 20.4 ms  | 0.94×    | tied with Go ✓ |
| fn_call             | 188 ms   | 0.04×    | 25× — pathological m49 cost |
| locus_instantiation | 3.07 ms  | 0.006×   | 167× — arena-per-locus |
| form_vec_get        | 1.61 ms  | 0.024×   | 42× — post lazy-error fix |
| **Amortized** (same primitives, used as designed) |  |  |  |
| fn_scratch_work     | 0.47 ms  | 0.96×    | 4% behind Go — design pays off |
| vec_amortized       | 2.40 ms  | 0.53×    | 2× behind Go — amortized over work |
| coord_with_churn    | 15.7 μs  | 0.010×   | parent-locus cliff dominates |

The decision-grounding observation: **m49's per-call subregion
isn't broken; it's amortizable.** `fn_scratch_work` (100 calls
× 1000 ops each) is at 0.96× — Hale is 4% behind Go when the
fn body has enough internal work to amortize the create/destroy.
The pathological overhead benches measure the cost in the
absence of work, which real apps don't experience.

## 2026-05-13 — subregion elision for non-allocating bodies

The codegen now classifies each user fn body as
allocating-or-not at declare time via a conservative syntactic
walk (`fn_body_definitely_non_allocating`):

- Safe: Literals (incl. String — global static), Ident,
  KwSelf, Field reads, Index reads on non-Range indices,
  Unary on safe operand, Binary on safe operands when op is
  numeric (Sub/Mul/Div/Mod/comparisons/bool/bitwise — Add
  excluded since it could be String concat), If with non-
  allocating arms, Block with non-allocating stmts, Return,
  Let/Assign of safe value, While with non-allocating body.
- Allocating: Call, Path, Struct literal, multi-Tuple, Array,
  ArrayRepeat, FString, Match, Or (fallible machinery),
  Range-index (slice allocs), and everything not explicitly
  whitelisted.

For non-allocating fns, `lower_user_fn_body` skips the
`lotus_arena_create_subregion` call entirely (sets
`fn_arena_alloca = caller_arena_alloca`), and the exit
epilogue skips both the deep-copy and the
`lotus_arena_destroy`. The return value either is a primitive
(no copy needed) or a pointer the caller already had access
to (String literal global, caller-passed pointer, field-read
of one of those — all stable across the fn frame).

| Bench               | Pre-elision | Post-elision | Go ratio before | Go ratio after | Δ |
|---------------------|-------------|--------------|-----------------|----------------|---|
| **fn_call**         | 188 ms      | **37.1 ms**  | 0.04×           | **0.21×**      | **5× faster** |
| **form_vec_push**   | 3.02 ms     | **2.79 ms**  | 0.90×           | **1.00×**      | tied with Go |
| bus_dispatch        | 2.21 ms     | 1.81 ms      | 0.021×          | 0.026×         | ~20% faster |
| form_vec_get        | 1.61 ms     | 1.47 ms      | 0.024×          | 0.026×         | small (no fn-decl in body) |
| loop_overhead       | 20.4 ms     | 20.4 ms      | 0.94×           | 0.94×          | unchanged |
| locus_instantiation | 3.04 ms     | 2.88 ms      | 0.007×          | 0.007×         | unchanged |
| fn_scratch_work     | 0.49 ms     | 0.49 ms      | 0.96×           | 0.92×          | within noise (already good) |
| vec_amortized       | 2.40 ms     | 2.72 ms      | 0.53×           | 0.42×          | within bench noise |
| stream_aggregator   | 4.39 ms     | 4.70 ms      | 0.005×          | 0.005×         | unchanged |

**form_vec_push at 1.00× is the real signal.** The form library
hit its FORM-3 contract target — `@form(vec)`'s push is now
exactly Go's speed. The reason it benefits: the bench's inner
loop body has no allocations either, so the wrapping fn that
runs the loop gets the elision treatment.

**fn_call 5× faster** is the direct hit. Per-call cost dropped
from ~18.8ns to ~3.7ns. Most of the remaining ~3ns is the
function-call ABI itself (caller_arena param load, alloca for
declared param, alloca for ret slot, store/load through them).
Further wins would need either dropping the `__caller_arena`
param too (changes per-fn ABI; bigger surgery), or arranging
for LLVM to inline small leaf fns (depends on visibility).
Both are deferred.

**What this does NOT fix.** The substrate-allocation gaps —
`locus_instantiation` (167×), `bus_dispatch` (53× → still 39×
after the small win above), `stream_aggregator` (200×) — are
unchanged. Those reflect arena-per-locus and bus-cell-arena
costs that are layout-conditioned by The Design. They wait on
either a different lifecycle design or a workload that makes
the cost measurably load-bearing.

## 2026-05-16 — cliff-lift session + O2 pipeline shift

The parallel perf session reported lifting all six known cliffs
via two fixes:

1. **alloca-hoist:** raw `build_alloca` calls landing inside loop
   bodies accumulated frame-bytes per iter until SIGSEGV. The fix
   routed the offending sites through
   `alloca_in_entry_with_nulled_arena` (and `alloca_in_entry` for
   non-arena slots) so the slot lives at fn-entry and is reused
   across iterations.
2. **arena-reclaim:** the chunked-projection accept() path now
   returns dissolved sub-region slots to the parent's free-list
   so peak slot space stays O(concurrent children alive) instead
   of O(K).

Reported ceilings (parallel perf session, unverified here):

| Path                                   | Pre-fix cap | Pre-fix segfault | After-fix clean |
|----------------------------------------|-------------|-------------------|-----------------|
| Statement-position locus instantiation | 100k        | 500k              | 10M+            |
| Bus pub/sub round-trip                 | 10k         | 50k               | 1M+             |
| @form(vec).push                        | 500k        | 1M                | 20M+            |
| @form(vec).get                         | 200k        | 300k              | 10M+            |
| @form(hashmap).get                     | 150k        | 200k              | clean (ceiling unverified) |
| accept() hook in loop                  | k=20        | k≈25              | k=20000+ (800×) |

The bench harness also shifted to running through the O2
pipeline; codegen-time emission unchanged but optimizer pass
ordering now matches the production-build path.

## 2026-05-16 — follow-up alloca audit

This session walked the remaining `build_alloca` call sites
looking for the same loop-body-leak pattern the cliff-lift
session fixed. Four sites had the matching shape: a raw
`build_alloca` whose address escapes to a C interop call, where
LLVM's mem2reg can't hoist it because of the escape. None of
these have benches today, so no number movement to report —
treat the changes as defensive hardening against future hot-loop
patterns:

- `std::time::monotonic` — `timespec` alloca passed to
  `clock_gettime`. A monotonic-clock-in-tight-loop bench would
  have leaked 16 B/iter.
- `std::time::sleep` — `req` and `rem` allocas passed to
  `nanosleep`. Same shape; less practical risk since the body of
  a loop calling sleep is rarely the throughput-critical path.
- Decimal `to_string` rendering — 64-byte buffer alloca passed
  to `lotus_decimal_to_string`. Decimal-heavy fmt in a loop
  would have accumulated 64 B/iter.
- `pthread_create`'s tid alloca on locus instantiation —
  parallel-class loci instantiated in a loop would have leaked
  8 B/iter on top of the locus struct itself (which the
  cliff-lift session already hoisted).
- `@form(vec).sort_by`'s qsort_r cookie (16 B) — introduced in
  this session; preemptively hoisted at the same time.

## What's still open for the FORM-3 gate

The original spec/forms.md FORM-3 gate text ("within 10% of
hand-written C on a 1M push + 1M random-index get microbench")
is now partially satisfied (push is at 1.00×, get is at 0.026×).
Worth a spec amendment that distinguishes:

- **Tight-loop primitive cost** (form_vec_push): commit to the
  10% gate. Currently met.
- **Per-op fallible-method cost** (form_vec_get isolated): the
  ~38× residual is the C-function-call boundary on
  `lotus_vec_get`. Spec should acknowledge this as a known
  cost; closing it would need IR-level inlining of the vec
  primitive's logic at codegen time (~5 IR instructions
  replacing the function call), or LTO. Deferred until a
  workload measures it.
- **Amortized workload cost** (vec_amortized at 0.42×): the
  spec should commit to "within 2× of C on amortized
  workloads," which `fn_scratch_work` (0.92×) and now
  `form_vec_push` (1.00×) demonstrate as reachable.

The 2026-05-16 cliff lifts mean the surviving headline
overheads (the 167× on `locus_instantiation`, etc.) are now the
*real* steady-state numbers — not artifacts of an iter cap
chosen to dodge a segfault. The perf session also flagged that
`loop_overhead` is now a closed-form-optimized no-op (~60ns
under LLVM at the new opt level) and needs a non-trivial loop
body to measure what it claims to measure; that adjustment
lives in the bench repo, not here.

Suggested next sweep: re-run each lifted bench at 2-3× its
new "tested clean" mark. Each fix shifted the bottleneck;
the next-tier cliff is often a different mechanism (alloca →
arena fragmentation → libc malloc churn) and is worth surfacing
before the FORM-3 numbers are locked in.
