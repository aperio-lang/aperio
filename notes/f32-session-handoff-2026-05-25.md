# F.32 session handoff — 2026-05-25

Session-state handoff for a new Claude session picking up
F.32 cache-aware substrate work + the cross-language bench
PR work. Delete this file once the next session has read it
and the deferred work is unblocked / superseded.

## Read first (onboarding chain)

1. **[`CLAUDE.md`](../CLAUDE.md)** — repo entry point. Says
   AGENTS.md is the canonical agent prompt.
2. **[`AGENTS.md`](../AGENTS.md)** — load-bearing prompt for
   `.hl`-authoring agents. Read for the language model + the
   patterns catalog.
3. **[`agents/compiler-dev.md`](../agents/compiler-dev.md)** —
   compiler / runtime / spec work brief. Read for the
   pipeline + spec discipline. The session's work is all
   compiler-side.
4. **[`notes/f32-cache-aware-delivery-plan.md`](./f32-cache-aware-delivery-plan.md)**
   — F.32 delivery plan. Source of truth on what shipped,
   what's deferred, and per-item scope. Updated through this
   session's work.
5. **[`notes/open-questions.md`](./open-questions.md)** —
   deferred design questions. Item #24 (fallible on user-
   declared locus member fns) was authored in this session.

## What shipped this session

Across 2 repos (`hale-lang/hale` + `hale-lang/bench`), all
pushed to GitHub `main`. Each commit's body has detailed
notes — read those for the per-deliverable design.

| Repo | Commit | What |
|---|---|---|
| hale | `e522d6e` | `claude.md`: surface LLVM 18 prereq + single-test idiom + flag missing `apps/` |
| hale | `1aed8fc` | F.32-{0, 1α, 4-prefetch, 4a, 4c} initial ship + plan amendment |
| hale | `8cf22d2` | F.32-1γ-v1: lockfree CAS striped hashmap — **1.30× faster than α** |
| hale | `c61e955` | F.32-1b: locus struct field reorder by access frequency |
| hale | `ed4cdee` | open-questions #24: scoping pass — codegen plumbing revised to ~500 LOC |
| hale | `6ce615e` | F.32-4-prefetch: `LOTUS_DISABLE_PREFETCH` build-flag A/B toggle |
| hale | `71a3d7e` | docs: document F.32 cache-aware env vars + sync disciplines |
| bench | `b1d5059` | F.32: 3 bench triples (Hale + Go + Node + Python) + cross-language grid |
| bench | `ce80fcd` | F.32-1γ-v1: flip `form_hashmap_false_sharing` to `sync = lockfree` |

**Headline numbers** (AMD Ryzen 7 9800X3D / x86_64 / Linux
6.18, `form_hashmap_false_sharing` bench, 200k cross-pool
concurrent inserts):

| Discipline | Elapsed | vs α |
|---|---:|---:|
| γ-v1 lockfree | **11.95 ms** | **0.77× (1.30× faster)** |
| α serialized | 15.51 ms | 1.00× |
| β2-v2 striped | 28.37 ms | 1.83× slower |

Hale vs Go's `sync.Mutex` map: closed from 1.66× (α) to
**1.18× (γ)** — closest Hale's ever been on this bench.

**Bonus win** (not F.32 scope but discovered + fixed):
`key_at`/`entry_at` O(N²) bug. Each call was O(N) probe →
loop was O(N²). Fixed with a monotonic-iteration cursor
on `lotus_hashmap_t`. `form_hashmap_walk_large` bench:
**6.88 s → 1.15 ms (6,140× speedup)**. Hale was 17,000×
behind Go on this bench; now within 3×.

## Honest findings (things that surprised us)

1. **β2-v2 striped is SLOWER than α** on 2-core / cheap-
   payload workloads (~1.87×). The per-op rwlock+CAS
   overhead exceeds the parallel-writer gain when per-op
   work is cheap. β2-v2's win materializes on 4+ cores or
   heavier per-op work; the `spec/forms.md` discipline-
   picker table documents this.

2. **F.32-4-prefetch contribution is below measurement noise**
   on Ryzen 9800X3D (huge 96MB L3, fast interconnect — the
   producer's write is already in L1/L2 by the time the
   consumer reads). Median with/without prefetch overlap.
   The hint is still cheap (no regression) and the
   `LOTUS_DISABLE_PREFETCH` build flag is shipped as a
   measurement tool for other hardware where smaller L2 or
   slower interconnect could show the originally-predicted
   10-50 ns/cell saving.

3. **HUGE_PAGES validation can't run on this host**: 0 huge
   pages reserved (`/proc/sys/vm/nr_hugepages` = 0). Requires
   `sudo sysctl vm.nr_hugepages=N` before the bench can
   exercise the path.

## Deferred work (each is multi-hour focused)

| ID | Item | Scope | Why deferred |
|---|---|---:|---|
| F.32-2 | Compile-time working-set budget per locus | 500-800 LOC | New analysis pass; large standalone deliverable. Builds on F.32-1b's field-access counter. |
| F.32-1∞ | Closed-world sync inference | ~200 LOC orchestration | Needs mutable Bundle thread between `check_bundle` and codegen. F.32-0's diagnostic delivers most of the ergonomic win today. |
| F.32-3 | Codegen-aware per-pool chunk inference | ~150 LOC | Env-var version (`LOTUS_ARENA_CHUNK_BYTES_OVERRIDE`) ships and covers the operational case. Codegen-aware version's win is bounded; no current bench exercises multi-locus-per-pool. |
| F.32-1γ-v2 | Lockfree grow + tombstones (remove support) | 3-5 sessions | γ-v1 fixed-cap + no-remove ships. v2 adds Cliff Click state machine + tsan/relacy validation. |
| #24 | `fallible(E)` on user-declared locus member fns | ~500 LOC codegen | Initial estimate of ~150 LOC was wrong; locus method call dispatch tracking fallibility per-method is the bulk. Open-questions.md #24 has the revised plan. |

The plan doc has each deliverable's preserved design in full
("kept verbatim below as the implementer's guide"). Read
those sections before starting any of them.

## Where the design intent lives

| Topic | File |
|---|---|
| F.32 plan + scope decisions | `notes/f32-cache-aware-delivery-plan.md` |
| Open design questions + #24 | `notes/open-questions.md` |
| Sync disciplines + picker | `spec/forms.md` § "Cross-pool sync disciplines" |
| Two-channel rule + cross-pool | `spec/types.md` § "Single-threaded-method invariant (F.31)" |
| Cache-aware env vars (operator-facing) | `docs/src/how-tos/keeping-memory-bounded.md` § "F.32 cache-aware env vars" |
| Bench cross-language grid + maintenance | `../bench/README.md` § "Cross-language comparative grid" |
| C twin establishing the theoretical max | `experiments/f32-false-sharing/` |

## Pick-up suggestions

In rough priority order:

1. **F.32-1b loop-weighting follow-up** — small (~30 LOC). The
   current reorder counts lexical occurrences uniformly. Add
   a 10× multiplier per loop-nesting level around each
   `self.<field>` access. Low risk, real perf bump on hot-
   loop-heavy bodies. The walk in `count_self_field_accesses_in_locus`
   in `crates/hale-codegen/src/codegen.rs` is the place; pass
   a `loop_depth` counter through the walk.

2. **F.32-3 codegen-aware per-pool chunk inference** —
   medium (~150 LOC). The placement-block walk exists from
   F.31; count loci per non-main cooperative pool, compute
   chunk-size hint, emit `lotus_arena_create_sized(hint, ...)`
   at instantiation. Worth shipping IF a downstream workload
   surfaces multi-locus-per-pool.

3. **F.32-1γ-v2 lockfree grow** — large (3-5 sessions).
   Tombstones + Cliff Click state machine. Worth pursuing
   once a real downstream workload hits the fixed-cap
   ceiling. Until then, γ-v1's `cap = N` is sufficient for
   the Prometheus-counter shape it was sized for.

4. **F.32-2 working-set budget** — large (500-800 LOC).
   Genuinely valuable for HFT-grade deploy gating; gates
   itself on `--target-cache=lN` opt-in so the runtime cost
   is zero when unused.

5. **#24 fallible member fns** — large (~500 LOC codegen).
   Real friction documented across multiple apps + libs; the
   typecheck change is trivial but the codegen plumbing is
   the bulk. Worth attacking when the friction reaches a
   tipping point.

## Things that won't work as documented

- **`apps/`** referenced in `AGENTS.md` + `README.md` doesn't
  exist in this repo. Use `crates/hale-codegen/tests/fixtures/examples/`
  (151 in-tree `.hl` programs) as the canonical corpus
  instead. CLAUDE.md already calls this out.

- **Bench harness `--bench=X` flag** — only the LAST `--bench=`
  on the command line takes effect (single-bench filter,
  not multi-select). To run multiple specific benches, run
  the harness multiple times.

- **`HALE_BIN` env var** — the bench's `run.sh` checks this
  before falling back to `../hale/target/release/hale`. If
  you've installed `hale` on PATH, the harness uses that
  instead — be aware which build is being measured.

- **`./run.sh --no-build`** — skips both the Hale binary
  rebuild AND each sibling's `go build`. If sibling
  binaries weren't pre-built, they're silently skipped.
  First-time runs need to omit `--no-build` to build the
  Go siblings.

## CI state

`hale-lang/hale` CI runs `tests` job (~10-16 min, partitioned
4-way via `cargo nextest`). Each push triggers a new run + the
in-flight one for the prior commit is cancelled per the
`ci: cancel in-flight release runs on tag re-push` workflow
shipped in commit `157c001`. Latest CI green: γ-v1
(`8cf22d2`). Subsequent commits all push the cumulative state;
the latest run validates everything.

`hale-lang/bench` has no CI configured.

## Verbose commit messages

Each session commit's body documents:
- What it shipped
- The C / Rust file paths touched
- Measured perf numbers (if applicable)
- What's deferred + why

`git log -1 --format=%B <hash>` on any session commit gives the
full picture without needing to re-derive from the diff. Same
discipline going forward; prefer detailed bodies over terse
one-liners since this work touches load-bearing substrate.
