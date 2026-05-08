# Lotus — session checkpoint

**Read this first** if you're picking up the lotus language work in a
new session. State as of commit `206fbd0` (2026-05-08).

This is part of the alpha-conjecture program (see
`~/notes/alpha-conjecture/CLAUDE.md`). Lotus is the language-substrate
arm — a programming language whose primitives are the framework's
coordination primitives.

## Where we are

A working compiler that **runs** lotus programs end-to-end (tree-
walking interpreter) AND **produces** native ELF binaries (LLVM via
inkwell) for a substantial subset including loci with `run()`
lifecycle methods. 86 tests pass across the workspace.

```
$ lotus run examples/hello-world/main.lt        # interpreter path
hello, world

$ lotus build examples/hello-world/main.lt      # codegen path
built: examples/hello-world/main
$ ./examples/hello-world/main
hello, world
```

Phase status:
- **Phase 0** (spec stabilization) — complete
- **Phase 1** (lex / parse / typecheck) — complete; F.1–F.18 enforced
- **Phase 2 v0** (interpreter + bus router) — 8 of 9 example projects
  execute end-to-end
- **Phase 3 milestone 7** (codegen subset) — complete; literals,
  arithmetic, let / let mut bindings, assignment + compound ops,
  mixed-type println, if/else/while + break/continue,
  `time::sleep` on CLOCK_MONOTONIC, user-defined fns, and the
  **locus runtime ABI**: each locus → LLVM struct, lifecycle
  methods take `self_ptr`, `self.X` reads/writes via
  getelementptr, statement-level instantiation does
  alloca → fill defaults+overrides → call birth → call run.
- **Phase 3 next** — `accept()` / `drain()` / `dissolve()`
  lifecycle methods (parent-child + recovery primitives), then
  `time::now()` / `time::monotonic()` value-returners, then the
  bus router lowering. After accept lands, `02-parent-child`
  becomes the next visible build-target.

## What runs vs. what builds

| Primitive | Interpreter | Codegen |
|---|---|---|
| `fn main()` entry | ✅ | ✅ |
| Int / Float / Bool / String literals + params | ✅ | ✅ |
| `let` bindings | ✅ | ✅ |
| Arithmetic, comparisons, logical ops | ✅ | ✅ |
| `self.X` reads (in lifecycle methods) | ✅ | ✅ (runtime GEP+load) |
| Locus instantiation + `birth()` | ✅ | ✅ (ephemeral only) |
| Mixed-type println (single printf) | ✅ | ✅ |
| `let mut` + assignment (incl. compound `+=` etc.) | ✅ | ✅ |
| `if` / `else` / `else if` / `while` + `break` / `continue` | ✅ | ✅ |
| `time::sleep` on CLOCK_MONOTONIC + EINTR retry | ✅ | ✅ |
| User-defined fns called from main / each other | ✅ | ✅ |
| `run()` lifecycle method | ✅ | ✅ |
| `self.X = ...` mutation in lifecycle methods | ✅ | ✅ |
| `for` / `match` | ✅ | — |
| `accept()` / `drain()` / `dissolve()` lifecycle methods | ✅ | — |
| Bus router (`<-` send + subscribe dispatch) | ✅ | — |
| Closure runtime (collapse / absorb / bubble) | ✅ | — |
| Modes as methods | ✅ | — |
| Recovery primitives (bubble) | ✅ | — |
| Recovery primitives (restart / quarantine etc.) | parsed | — |
| Region allocator (per-projection-class arenas) | — | — |
| Cooperative scheduler | — | — |

## Locked design commitments (F.1–F.18)

Spec source: `spec/design-rationale.md`. Summary:

- **F.1** k_max = B / [(1−φ)c + φσ] is the framework equation.
- **F.2** `ProjectionClass` as built-in any-of-three constraint.
- **F.3** Per-arena defrag/free-list, no whole-program GC.
- **F.4** `drain()` always cascades depth-first.
- **F.5** Mode projections share the locus's arena.
- **F.6** Lifecycle methods are not implicit loci.
- **F.7** `accept()` runs before child birth.
- **F.8** Contract compatibility type-checked across coordinator /
  coordinatee.
- **F.9** Collapse vs. explosion + parent on_failure routing
  (absorb / bubble).
- **F.10** Mode keywords accepted post-dot as member names.
- **F.11** `self.children` typing and lifecycle.
- **F.12** Bus send is `<-`; subscribe is declarative.
- **F.13** Bus subscription handler signature.
- **F.14** Three-way interface: locus + parent + contract.
- **F.15** Predefined type names are PascalCase, not keywords.
- **F.16** `self.k_max` as built-in computed field (F.1 executable).
- **F.17** Strict field-access; method types on locus / perspective.
- **F.18** Match exhaustiveness checked at typecheck.

## Files to read for orientation

In order:

1. `README.md` — overview, status, F-table, examples, toolchain.
2. `spec/design-rationale.md` — why each construct is shaped the way
   it is. Source of truth for F.1–F.18.
3. `spec/grammar.ebnf` — formal grammar.
4. `spec/tokens.md` — lexical structure.
5. `spec/precedence.md` — operator precedence table.
6. `examples/hello-world/main.lt` through `examples/trellis-demo/main.lt` —
   the example ladder. trellis-demo exercises the full pipeline.
7. `crates/lotus-syntax/src/lib.rs` — public API of the parser/AST.
8. `crates/lotus-types/src/lib.rs` — typechecker entry + unit tests
   that lock the F.x rules.
9. `crates/lotus-runtime/src/lib.rs` + `eval.rs` + `bus.rs` —
   interpreter, dissolve cascade, bus router.
10. `crates/lotus-codegen/src/codegen.rs` — current LLVM lowering.
11. `crates/lotus-cli/src/main.rs` — CLI dispatch.
12. `~/.claude/plans/witty-foraging-lightning.md` — the original
    delivery plan to team-wide internal v1.0 (~18–30 months total).

For broader program context:

- `~/notes/alpha-conjecture/CLAUDE.md` — the master project guide.
  Lotus is one substrate-arm among several; paper 4 is the program's
  foundational anchor (read its memory file too).
- `~/notes/alpha-conjecture/lotus/` — the design-time meta-framework
  that lotus-the-language is the compile-time projection of.

## Strategic preferences locked in

These are user (Riley) directions saved into auto-memory at
`~/.claude/projects/-home-riley-notes-alpha-conjecture/memory/`:

- **Greenfield cleanup as we go** — pre-ship code is greenfield;
  drop "preserved old behavior" / fallback patterns; clean up
  rather than accumulate compatibility cruft. (See
  `feedback_greenfield_cleanup.md`.)
- **Stay focused on lotus** for the foreseeable session — don't
  swing back to paper-4 / theory work without explicit redirect.
- **LLVM is the codegen target** — committed; toolchain installed
  (llvm-18 + clang + lld + libpolly-18-dev). inkwell 0.5 +
  llvm-sys 180.0.0 against system LLVM.
- **Trellis informs but doesn't dictate** — production trellis-pair
  (analyst/executor as separate binaries) is the eventual real-world
  use case, but we're not building specifically toward it. It's a
  milestone we'll hit when the pieces are right; for now,
  `examples/trellis-demo/` is the single-process surrogate that
  exercises the full pipeline.

## User context (Riley)

Junior partner at small finance firm. Deep software-architecture
expertise via brain3 (production deployment at the firm,
brained.dev). The trellis trading system is the natural first
real-world use case for lotus.

## Recent commit history (last 30, newest first)

```
206fbd0 Codegen milestone 7: locus runtime ABI
79ae75f CHECKPOINT.md: refresh for milestone 6
9955bea Codegen milestone 6: multi-fn programs
29c8bdf README + open-questions: sync to milestone-5 state
fd53a6d CHECKPOINT.md: refresh for milestone 5
929efa2 Codegen milestone 5: time::sleep on CLOCK_MONOTONIC
cd01f9a CHECKPOINT.md: refresh for milestone 4
cae8c9a Codegen milestone 4: if / while / break / continue
76992f1 CHECKPOINT.md: refresh for milestone 3
03c2f55 Codegen milestone 3: let mut + assignment
5224d53 Codegen milestone 2: let + Int/Float arithmetic + comparisons
5c9b6f7 Codegen milestone 1: Int / Float / Bool params + mixed-type println
77b977f Phase 3 milestone 0: lotus build → native ELF via LLVM
4b5b00c Spec sync: F.16 / F.17 / F.18 added; F.8 / F.9 / closure refined
ed81e56 Match exhaustiveness check at typecheck
34c188f F.1: self.k_max as computed field on locus values
6e630e1 Closure-cycle existence check: reject pure-literal assertions
dd325fe Strict field-access checking + locus/perspective method types
72c5036 F.8: contract compatibility checked across coordinator/coordinatee
13ba006 match expressions execute: literals, wildcards, bindings, tuples
2fe0ca9 Program-end dissolve: long-lived locus closures actually fire
c3dbe94 F.9 closes: collapse / absorb / bubble — three separate demos
22c27bf F.9 routing: parent on_failure absorbs ClosureViolation
c738e9e Closure-test runtime: F.9 collapse vs explosion fires
efe0358 trellis-demo: full pipeline runs end-to-end + Decimal arithmetic
bb1910e Bus: Transport trait + SyncDispatch + RingBuffer impls
ef752d9 v0 bus router: `<-` actually delivers; 05-bus runs end-to-end
e07b3ce Phase 2 v0: tree-walking interpreter — `lotus run` works
07c3e58 Phase 1 milestone 2: type checker (resolve + check passes)
8cc583b v0.1.8: PascalCase predefined types + bus-send `<-` operator
5a961f0 Phase 1 milestone 1: lex / parse / AST threaded through
```

40 commits ahead of origin/master at checkpoint time.

## Next steps in priority order

Conceptual locus depth (deepest = substrate-touching, shallowest =
user-facing). Each is a focused single-commit chunk unless noted.

**Codegen surface expansion (Tier 4, the LLVM path):**

1. **Parent-child lifecycle: `accept()` / `drain()` / `dissolve()`
   + child loci.** `accept()` takes the parent reference; child
   loci attach to a `self.children` slot. With this, `02-parent-
   child` becomes a build target. Touches the locus runtime ABI's
   "ephemeral-only" constraint — long-lived child loci need a
   region-allocator ancestor before they can stay alive past the
   surrounding fn return. Initial scope: scope-bounded parent
   holding stack-allocated children + drain cascade at scope exit.
2. **`time::now()` / `time::monotonic()`** — the value-returning
   side of the clock module. `clock_gettime(CLOCK_MONOTONIC)` and
   `clock_gettime(CLOCK_REALTIME)` lowering; pairs with the
   monotonic-only-scheduling discipline locked in by milestone 5
   (see spec/runtime.md "Time" section).
3. **Bus router lowering** — vtable-style dispatch, sync transport
   first; ring buffer follows. Depends on accept lowering.
4. **Modes (bulk / harmonic / resolution)** — share the locus's
   alloca'd struct with three projection-specific dispatch entry
   points.
5. **Closure runtime as a small C-runtime support library**
   (statically linked) — once we're ready to compile away from
   the interpreter for the closure-test path.
6. **`for` loops + arrays.** Need an array runtime representation;
   simplest is `{ i64 len, ptr data }` for fixed-size arrays
   first.

**Smaller follow-ups behind the locus ABI work:**
- `return n;` from main → process exit code (one-line lowering
  once the special-cased main path can lift `return`)
- Default param values on user fns (already in AST; declare time
  rejects them today)
- Locus param defaults that aren't literals (current constraint:
  literal-only at declare time; lift by deferring default eval to
  the instantiation site through `lower_expr`)

**Runtime side (Tier 0/1, deferred):**

- Region allocator (per-projection-class strategies)
- Cooperative scheduler (BEAM-shaped)
- Cross-process shared-memory ring buffer (production trellis-pair)
- Recovery primitives execution (restart / quarantine — needs
  scheduler + region allocator)

**Outstanding deferrals worth tracking:**

- Generic instantiation (record args, no substitution yet)
- Module / import resolution (parsed only)
- Tree-sitter grammar derivation from EBNF
- LSP server
- Self-hosting (Phase 6, distant)

## Toolchain state

System has:

- `llvm-config` 18.1.3 at `/usr/bin/llvm-config`
- `clang` 18.1.3 at `/usr/bin/clang`
- `lld` at `/usr/bin/lld`
- `libpolly-18-dev` (required by llvm-sys for static link)
- `gcc` 13.x

Cargo workspace builds clean. `cargo test --workspace --tests` passes
all 86 tests (the locus-with-run test runs 3×500ms sleeps so the
runtime + codegen integration buckets clock ~1.5s each).

## How to verify the checkpoint

```
cd ~/code/lotus-lang
cargo test --workspace --tests           # 86 passed
cargo run --bin lotus -- run examples/trellis-demo/main.lt
cargo run --bin lotus -- build examples/hello-world/main.lt
./examples/hello-world/main              # prints "hello, world"
rm examples/hello-world/main             # clean up artifact
cargo run --bin lotus -- build examples/01-locus-with-run/main.lt
./examples/01-locus-with-run/main        # tick 0..2 over 1.5s
rm examples/01-locus-with-run/main       # clean up artifact
cargo run --bin lotus -- build examples/06-mutable-counter/main.lt
./examples/06-mutable-counter/main       # prints "n=2"
rm examples/06-mutable-counter/main      # clean up artifact
cargo run --bin lotus -- build examples/07-control-flow/main.lt
./examples/07-control-flow/main          # prints "sum=29 stopped at n=9"
rm examples/07-control-flow/main         # clean up artifact
cargo run --bin lotus -- build examples/08-monotonic-sleep/main.lt
./examples/08-monotonic-sleep/main       # prints tick 0..2 + done; ≥150ms
rm examples/08-monotonic-sleep/main      # clean up artifact
cargo run --bin lotus -- build examples/09-functions/main.lt
./examples/09-functions/main             # prints square(7)=49 / fib(12)=144 / ...
rm examples/09-functions/main            # clean up artifact
cargo run --bin lotus -- build examples/10-stateful-locus/main.lt
./examples/10-stateful-locus/main        # prints total=160 / step=30
rm examples/10-stateful-locus/main       # clean up artifact
```

If all ten work, the checkpoint is intact.
