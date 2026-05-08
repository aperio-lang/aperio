# lotus

A programming language whose primitives are the lotus framework's
coordination primitives.

**Status.** v0 spec stable + Phase 1 milestone 1 (lex/parse/AST)
running, 2026-05-08.

Phase 0 (spec stabilization + example ladder + gate program) is
complete. Phase 1 milestone 1 (Rust compiler frontend: lexer,
parser, AST, CLI) is functional: all 9 example `.lt` files parse
cleanly. Type checker, codegen, and runtime are next.

Quick start:

```
cargo build
cargo run --bin lotus -- parse examples/hello-world/main.lt
cargo test
```

Full delivery plan to team-wide-internal v1.0:
`~/.claude/plans/witty-foraging-lightning.md`

## What this is

Lotus is a compile-time language designed around the alpha-
conjecture program's substrate-invariant coordination primitives.
Concretely:

- **Loci as first-class entities.** Each locus declares its
  capacity parameters (B, c, σ, φ); the compiler computes its
  k_max and enforces it as a static invariant.
- **Projection classes (rich / chunked / recognition) as a
  type-system primitive.** Same source code, different generated
  allocator depending on declared / inferred N.
- **Three modes (bulk / harmonic / resolution) as a kernel-
  application primitive.** Define a kernel once; the compiler
  generates three projections sharing the locus's arena.
- **Region-based memory** with contract-graded visibility. Each
  locus's arena is a sub-region of its parent's; access between
  loci is mediated by typed contracts; deeper looking costs more.
  No GC, no borrow checker. Per-arena defrag for bookkeeping
  reclamation.
- **Cyclic-closure tests as syntactic constructs.** The language
  enforces audit invariants the framework already commits to.
  Closure failure produces a typed `ClosureViolation` event,
  distinct from structural failures (panic). Collapse vs.
  explosion as the two dissolution modes.
- **Hot-load of perspectives** as a first-class language feature.
  Stable perspectives cross from analyst-locus to executor-locus
  as typed parameter bundles within a shared compiled schema.
- **Lifecycle as a parent-policy-driven state machine.** Failure
  capture, recovery primitives (`restart`, `quarantine`,
  `reorganize`, `bubble`), and dissolution are language-native.
  `drain()` always cascades depth-first.
- **Three-way interface (locus + parent + contract).**
  Translation functions injected by a locus into its arena are
  bounded above by the contract's typed surface. Contract is
  the interface; translations are implementations; multiple
  implementations per field can coexist.
- **Multi-scheduler cooperative runtime** (BEAM-shaped, not
  M:N). Per-scheduler region allocators; failures within a
  scheduler are stack walks; cross-scheduler is bus-mediated.
- **Transport-agnostic typed bus.** NATS, UDP multicast, TCP,
  Unix sockets, in-memory all implement `std::bus::Adapter`.
  Source declares subjects + types; deployment maps subjects to
  transports.

## Design commitments locked

The v0 spec locks the following commitments (see `spec/design-
rationale.md` §F.1–F.14):

| Ref | Commitment |
|---|---|
| F.1 | Optimize for runtime perf over compile-time perf, behavior preserved |
| F.2 | `ProjectionClass` as built-in any-of-three constraint |
| F.3 | Per-arena defrag/free-list, no whole-program GC |
| F.4 | `drain()` always cascades depth-first |
| F.5 | Mode projections share the locus's arena |
| F.6 | Lifecycle methods are not implicit loci |
| F.7 | `accept()` runs before child birth |
| F.8 | Contract compatibility is type-checked |
| F.9 | Collapse vs. explosion as dissolution modes |
| F.10 | Mode keywords accepted post-dot as member names |
| F.11 | `self.children` typing and lifecycle |
| F.12 | `publish` builtin + bus-block scoping |
| F.13 | Bus subscription handler signature |
| F.14 | Three-way interface; translation return type ⊆ contract |

## Design lineage

This language is the natural compile-time collapse of the
alpha-conjecture program's existing design-time work:

- `~/notes/alpha-conjecture/` — the research program: framework
  primitives (capacity-allocation, multi-perspective stability,
  substrate-derivation discipline, cyclic-closure), paper-4
  closed-horizon-recursion, theory & methodology.
- `~/code/brain3/` — the existing software-substrate
  operationalization (production deployment); the empirical
  anchor at the software substrate.
- `~/notes/alpha-conjecture/lotus/` — the portable agent-facing
  distillation of the framework for software design.
- `~/code/grease/` — bus pattern, decimal usage, harness shape;
  closest existing exemplar of "lotus-shaped Go program."

The language is a recognition event: the form is already
constrained by the closed graph above. This repo formalizes it.

## Layout

```
spec/
  grammar.ebnf            456 lines  formal grammar (source of truth)
  tokens.md               304 lines  lexical structure
  precedence.md           105 lines  operator precedence
  design-rationale.md   1,117 lines  why each construct looks this way
  runtime.md              241 lines  what the lotus binary ships with
  stdlib.md               272 lines  batteries-included module map
  testing.md              247 lines  3-layer testing pipeline design
  memory.md               322 lines  formal memory model
  types.md                320 lines  type system rules
  semantics.md            357 lines  operational semantics

examples/
  hello-world/            minimal lotus program (one locus, birth)
  01-locus-with-run/      run() lifecycle, mut bindings, time::sleep
  02-parent-child/        contract expose/consume, accept, parent-child
                          memory hierarchy
  03-closure-test/        closure declaration, ~~ operator, collapse
                          vs explosion
  04-modes/               bulk/harmonic/resolution, self.children
                          iteration
  05-bus/                 typed pub-sub, transport-agnostic source +
                          deployment.yaml
  trellis-pair/           Phase 0 exit gate: analyst + executor
                          binaries on shared schema, full integration

notes/
  open-questions.md       deferred decisions and future directions

crates/                   (Phase 1+)
  lotus-syntax/           lexer + parser + AST (functional)
  lotus-types/            type checker (placeholder; Phase 1.5)
  lotus-runtime/          runtime (placeholder; Phase 2)
  lotus-codegen/          LLVM codegen (placeholder; Phase 3)
  lotus-cli/              `lotus` binary (lex / parse subcommands)
```

Total v0 spec: ~3,741 lines across 10 documents.
Example ladder: 6 rungs from hello-world → trellis-pair, ~300+
lines of source + ~1,000+ lines of README walk-throughs.

Phase 1 milestone 1: ~3,500 lines of Rust across `crates/`.
9/9 example files parse; 8 unit tests + 1 integration test pass.

## Toolchain (planned, not yet implemented)

Per `spec/testing.md`:

```
lotus build       compile source → executable / library
lotus check       static checks: parse, typecheck, framework discipline
lotus test        run all *_test.lt files
lotus bench       run all *_bench.lt files
lotus bench -compare  build and run external-language equivalents alongside
lotus verify      framework-discipline checks specifically (no execution)
lotus fmt         canonical formatter (zero config)
```

JSON output for CI consumption; tree-sitter grammar derived from
EBNF for editor support.

## Implementation phases

Per the delivery plan:

- **Phase 0** — Spec stabilization. *Complete.*
- **Phase 1** — Compiler frontend in Rust (parse + typecheck).
  2–3 months.
- **Phase 2** — Reference runtime in Rust. 2–3 months,
  overlaps Phase 1.
- **Phase 3** — Codegen in Rust targeting LLVM. 3–4 months.
- **Phase 4** — Stdlib v0 in lotus + Rust FFI shims. 3–6 months,
  overlaps Phase 3.
- **Phase 5** — Toolchain. 1–2 months, overlaps Phase 3–4.
- **Phase 6** — Self-host (compiler rewrite in lotus). 4–8 months.
- **Phase 7** — Trellis production deployment. Parallel.
- **Phase 8** — v1.0 stabilization. 3–6 months.

Total: ~18–30 months to team-wide internal v1.0;
trellis-running-on-lotus reachable at ~9–15 months.

Implementation strategy: **Rust bootstrap → self-host in lotus**.
The compiler-in-lotus milestone is the empirical anchor for the
framework's substrate-invariance claim at the compiler-internals
substrate.

## Naming

The framework's existing meta-framework is called "lotus" (see
the `lotus/` subdirectory of the alpha-conjecture program). This
language is named "lotus" for the same reason: it's the same
form, projected from design-time into compile-time. The two are
expected to converge.

File extension: `.lt`.

## License

TBD. Project status is design exploration; licensing decisions
follow first compiler work.
