# Changelog

Behavior changes by release. The canonical spec lives in
[`spec/`](./spec/) — each file there represents *current*
behavior.

---

## v0.8.3 — CQRS at the locus boundary + F.34 windowed closures

A small release focused on closing structural-design rules at
typecheck. Three compile-time rejections previously only
discoverable as silent runtime behavior become hard errors; one
new closure-clause closes the rate-budget gap; one substrate
diagnostic closes the silent-drop debug gap.

### Language surface

- **CQRS at the locus boundary (#18.6 / #81).** Methods on loci
  may not return locus values. The compiler rejects
  `fn lookup(id: String) -> Counter` on a registry locus at
  typecheck. The rule keeps the substrate model honest — a
  returned locus would be a stranger in the caller's scope, with
  no lifecycle tower above it. Three canonical alternatives:
  parent-child + contract (`accept`'d children, pair with an
  index slot for name-based lookup), bus topic (publish typed
  commands keyed by name), or delegation (collapse the per-child
  operation onto the parent). See `spec/semantics.md § Locus
  method dispatch`.

- **`resets_per_epoch(...)` closure clause (F.34, #75).**
  Closes the `low_corrupt_rate`-shaped friction (per-window rate
  budgets). A closure paired with `epoch duration(N)` may now
  declare `resets_per_epoch(field1, field2, ...);` — the
  runtime zeros the named fields AFTER the assertion fires at
  each duration boundary. Ordering matters: the assertion sees
  the window's accumulated value, the reset prepares the next
  window. Typecheck rejects pairing with non-duration epochs and
  non-numeric fields. See `spec/semantics.md § Per-epoch field
  reset` + `spec/design-rationale.md § F.34`.

  ```hale
  closure low_corrupt_rate {
      self.corrupt_per_min ~~ 0 within 10;
      epoch duration(1m);
      resets_per_epoch(corrupt_per_min);
  }
  ```

- **Nested long-running cooperative children rejected at typecheck
  (#76 / F.31-followup).** A non-main locus with a non-trivial
  `run()` body holding a `params` field of a locus type whose own
  `run()` is also non-trivial — including `std::http::Server` and
  the other entries on the known-long-running stdlib allowlist —
  is now a compile error pointing at the sibling-in-main +
  placement fix. The runtime starvation that motivated this rule
  was silent (parent's `run()` simply never executed), so the
  type-side rejection converts a class of hard-to-diagnose
  runtime bugs into a clear compile-time signal. See
  `spec/runtime.md § Long-running cooperative children`.

### Diagnostics

- **`@form(hashmap)` cell-locus rejection improved (#77).** The
  pre-existing rule (cells may not be locus references) now
  produces a diagnostic that names the three canonical
  alternatives (parent-child + index, bus topic, delegation) and
  cross-references `spec/semantics.md § Locus method dispatch`.
  Same framing as #18.6 at the form-synthesis layer.

- **`LOTUS_BUS_LOG_DESERIALIZE_DROP=1`.** Surfaces silent drops
  in the udp:// reader thread when no deserializer is registered
  for the inbound subject, or the deserializer returns ≤ 0 (size
  mismatch, bounded-read failure). Off by default; the silent-skip
  on cross-routed multicast noise stays correct in steady state.
  Three udp:// bring-up handoffs this week traced back to silent
  drops on `deserialize → local-dispatch`; the lack of any signal
  was load-bearing on debug cycles. Same env-gated pattern as the
  existing `LOTUS_BUS_LOG_UNMATCHED`.

### Internals

- **Codegen refactor (#22).** `crates/hale-codegen` reorganized:
  per-domain submodules (`locus/`, `bus/`, `shared/`, `stdlib/`),
  `codegen.rs` reduced by 56.2%. No surface-level changes.

### Documentation

- **`docs/src/concepts/the-locus.md`** — CQRS rule paragraph.
- **`docs/src/concepts/the-bus.md`** — routing keys +
  `on_unmatched` policies (covering machinery shipped in v0.8.2).
- **`docs/src/concepts/capacity-storage.md`** — hashmap cell-
  locus rule with alternatives.
- **`docs/src/concepts/error-handling.md`** — `resets_per_epoch`
  coverage in the closures intro.
- **`docs/src/how-tos/threading.md`** — nested-long-running
  rejection in "What you can't do".
- **`docs/src/how-tos/keeping-memory-bounded.md`** — factory /
  cached-handle sections rewritten around the boot-time Int-
  index resolution pattern (the previous example used the
  now-rejected `reg.counter().inc()` shape).
- **`spec/design-rationale.md`** — new F.34 entry.

---

## v0.8.1 — F.32 cache-aware substrate + #24 narrowing

Cumulative changes since v0.8.0. No source-level breaking
changes; one rule narrowing (open-question #24) lifts a
previous restriction.

### Language surface

- **`fallible(E)` on user-declared locus member fns**
  (open-question #24). The blanket "locus methods cannot
  declare `fallible(E)`" rule narrowed to "substrate-facing
  surfaces cannot." User-declared `fn` member fns now
  carry `fallible(E)` like free fns do, with the full `or
  raise` / `or <substitute>` / `or <handler(err)>` /
  `or discard` disposition surface. Heap-bearing success
  and err payloads (`String`, `Bytes`, nested-struct-with-
  heap-fields) are supported via the same TLS caller-arena
  snapshot non-fallible heap-returning locus methods use.

  Still rejected (substrate-facing surfaces, no caller
  frame to address the value channel): lifecycle methods
  (`birth` / `run` / `accept` / `drain` / `dissolve` /
  `on_failure`), mode methods (`bulk` / `harmonic` /
  `resolution`), closure assertions, and bus-subscribed
  handlers. Bus-handler rejection fires at the subscribe
  site, not the fn decl. See `spec/semantics.md`
  § "Where each channel lives".

- **`@locality(L1|L2|L3|any)` annotation on a locus**
  (F.32-2 v0.2). Pins a per-locus cache-tier budget the
  working-set estimator evaluates against. `any`
  explicitly opts out of any global gate. Stacks with
  `@form(...)` in either order; max one of each. See
  `spec/grammar.ebnf` § `locality_annotation` +
  `spec/types.md` § "Working-set estimator (F.32-2)".

### Cross-pool `@form(hashmap)` sync disciplines

The cross-pool exemption that admitted plain `@form(hashmap)`
loci into concurrent-write paths was found to corrupt the
runtime's hashmap on concurrent grow (`lotus_hashmap_set` /
`_grow` are non-atomic single-threaded code).

- **F.32-0**: cross-pool exemption reverted; plain
  `@form(hashmap)` is single-pool by default. Cross-pool
  use requires an explicit `sync = X` opt-in.
- **`sync = serialized`** (α): per-map mutex. Simplest
  correct cross-pool path.
- **`sync = striped`** (β2-v2): cell-level CAS + per-map
  rwlock for grow + cache-padded cells. Parallel writers;
  grow path serializes.
- **`sync = lockfree, cap = N`** (γ-v1): fixed-cap,
  cell-level CAS, no rwlock or mutex. Highest measured
  throughput on the false-sharing bench (1.30× over α at
  2 cores, AMD Ryzen 9800X3D); no grow, no remove.

Discipline-picker table in `spec/forms.md` § "Cross-pool
sync disciplines". Inference (closed-world picks one of
α/β/γ from the pool-propagation graph) lands as a
typecheck-diagnostic enhancement; explicit pasting still
required to apply (auto-apply deferred).

### Working-set estimator (F.32-2)

Compile-time analysis projecting each locus's bytes
against a cache-tier budget. Opt-in via:

- **`hale build --locality-report`** — informational
  per-locus table on stderr; build proceeds.
- **`hale build --target-cache l1|l2|l3`** — over-budget
  loci warn on stderr; build proceeds.
- **`hale build --target-cache lN --strict`** — over-budget
  loci fail the build before codegen (exit 1).
- **Per-locus `@locality(...)`** — annotation wins over
  global `--target-cache`; `@locality(any)` opts out.

Tier sizes auto-detect from
`/sys/devices/system/cpu/cpu0/cache/index{0,2,3}/size` on
Linux (cached for the build's lifetime); static fallbacks
32 KB / 512 KB / 8 MB apply elsewhere.

Estimator accounts for alignment padding (struct interior
padding + final padding to struct alignment); previous
packed-layout assumption under-estimated by ~10-20% on
mixed-alignment shapes.

### Codegen substrate work

- **Codegen-aware per-pool chunk-size hint** (F.32-3).
  Loci instantiated on a non-`main` cooperative pool get
  a chunk-size hint sized to `target_L2_per_core /
  loci_on(pool) / typical_chunks_per_locus`, clamped to
  `[4K, 64K]`. The runtime's `lotus_arena_create_labeled_sized`
  honors the hint; env override
  (`LOTUS_ARENA_CHUNK_BYTES_OVERRIDE`) still wins via the
  upper bound.
- **Locus struct field reorder by access frequency**
  (F.32-1b). User-declared `params { }` fields are sorted
  by `self.<field>` access count, with a 10^depth
  multiplier per loop nesting level. Hot fields land on
  the first cache line of `self`.
- **Bus-dispatch prefetch hint** (F.32-4-prefetch). Producer
  emits `__builtin_prefetch(slot, 1, 3)` after the memcpy
  in `lotus_coop_pool_post` and friends. A/B toggle via
  `LOTUS_DISABLE_PREFETCH=1` at build time.
- **Huge-page-backed arenas + `mlockall`** (F.32-4a / 4c).
  Operator-tunable via `LOTUS_HUGE_PAGES=1` /
  `LOTUS_LOCK_MEMORY=1` env vars; documented in
  `docs/src/how-tos/keeping-memory-bounded.md`.

### Tooling

- **highlight.js mode** (mdbook docs site): `placement`
  and `discard` now style as keywords. `@locality(...)`
  picks up the generic `@<ident>` annotation rule.
- **heron tree-sitter grammar** (sibling repo): adds
  `placement_block` + `placement_spec` + `locality_annotation`
  + `locality_tier`. Editor highlighting + the future LSP
  parse both new constructs. Released as
  `hale-lang/pond@5d8202d`.

### Documentation

- **README** rewritten with substrate-pluralization framing:
  matchmaker example walkthrough (every phrase maps to a
  syntactic slot), "One language. Every substrate." section
  (native + browser shipped via hale-js; mobile / embedded /
  GPU / robotics / edge characterized as workload-pull, not
  roadmap), "Try it on code you already have" zero-install
  demo via AGENTS.md drop-in, "what the compiler is doing
  for you" enumeration with F.32 as receipt.
- **Spec sweep** for #24 + F.32-2: `spec/types.md`
  declaration restrictions narrowed and new "Working-set
  estimator" section; `spec/semantics.md` "Where each
  channel lives" rewritten; `spec/styleguide.md` two-channel
  rule references narrowed; `spec/stdlib.md` TCP Stream
  sentinel-shape framing updated; `spec/grammar.ebnf`
  picks up `locality_annotation`.

### Internals

- Sync inference walker covers all `Expr` arms
  (`Sum` / `Prod` / `Approx` / `Range` / `ArrayRepeat` / `Or`);
  previously the catch-all `_ => {}` arm under-counted
  `self.<field>` references inside closure assertions,
  range expressions, and `or`-substitute RHS.
- Working-set estimator's `BudgetBreach` records carry
  `tier: CacheTier` + `source: BudgetSource` so per-breach
  diagnostics name whether the contract came from
  `@locality` or `--target-cache`.

### Not in this release

The deliberately-deferred items per `notes/f32-cache-aware-delivery-plan.md`:

- **F.32-1γ-v2** (lockfree grow + tombstones). Needs tsan /
  relacy concurrency validation and a downstream workload
  that hits γ-v1's fixed-cap ceiling. Default: do not
  pursue until both gates clear.
- **Auto-applied sync inference**. The inference engine
  picks `sync = X` from the pool-propagation graph; v0.2
  will inject the kwarg into the AST so codegen honors
  it without the user pasting. v0.8.1 ships diagnostic
  enhancement only.
- **NUMA-aware placement** (`pinned(numa = N)`). No
  workload pulling yet.

---

## v0.8.0 — initial release

The language surface is stable. A few small additions are
planned, but most work from here to v1 is bugs, stability, and
performance — not new syntax or new semantics. Pin to a commit
if you build on it; small additions still land. The reference
contract is the spec under `spec/` plus the in-tree fixture
programs under `crates/hale-codegen/tests/fixtures/examples/`.
