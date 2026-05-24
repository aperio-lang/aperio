<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/assets/hale-banner-dark.svg">
  <img alt="Hale — hypergraph programming" src="docs/assets/hale-banner-light.svg" width="100%">
</picture>

[![Tests](https://github.com/hale-lang/hale/actions/workflows/tests.yml/badge.svg)](https://github.com/hale-lang/hale/actions/workflows/tests.yml)
[![Docs](https://github.com/hale-lang/hale/actions/workflows/docs.yml/badge.svg)](https://hale-lang.github.io/hale/)
[![License](https://img.shields.io/badge/license-Apache_2.0-blue.svg)](./LICENSE)
[![LLVM](https://img.shields.io/badge/LLVM-18-red.svg)](https://llvm.org/)
[![Status](https://img.shields.io/badge/status-stabilizing-blue.svg)](#status)
[![GC](https://img.shields.io/badge/GC-0-brightgreen.svg)](#state-of-the-culture)
[![async/await](https://img.shields.io/badge/async%2Fawait-0-brightgreen.svg)](#state-of-the-culture)
[![native](https://img.shields.io/badge/native-human_%2B_agent-8957e5.svg)](./AGENTS.md)

> **Language surface is stable.** A few small additions are planned.
> **v0.x — bug-fixing and stabilization work from here to v1.**

A language where the shape of your code matches the shape of your
thinking.

You know that feeling when you describe a system out loud —
*"a matchmaker holds a queue of waiting players, spawns a match when
enough are queued, then goes back to waiting"* — and then the code
you actually have to write bears no resemblance to those words?
Mutexes appear. Async machinery. Lifecycle wiring. Five files.

Hale is a bet that gap doesn't have to be there.

## A small program

```hale
type Player    { id: String; name: String; }
type MatchInfo { match_id: String; players: [Player]; }

topic JoinQueue  { payload: Player; }
topic MatchReady { payload: MatchInfo; }

@form(vec)
locus Matchmaker {
    params   { target_size: Int = 4; }
    capacity { heap waiting of Player; }
    bus {
        subscribe JoinQueue as on_join;
        publish   MatchReady;
    }

    fn on_join(p: Player) {
        self.waiting.push(p);
        if self.waiting.len() >= self.target_size {
            MatchReady <- assemble_match(self.waiting, self.target_size);
        }
    }
}
```

That's the whole service. The queue, the join handler, the
publish-when-full rule — one declaration, in the order you'd
describe it out loud.

## One shape, three substrates

The matchmaker decomposes the same way on paper, in a Hale
program, and inside an LLM's planning head. That's not an
accident.

When K things attach to one coordination point, the working
state needed to hold them together costs K log₂ K bits. The
ceiling is substrate-invariant — the same k̄ ∈ [4, 10] shows
up in human working memory, in management spans, in surgical
teams, in mixture-of-experts active counts, in multi-agent
LLM saturation. A Hale program is the literal shape of that
bound: loci are vertices, topics are hyperedges, capacity
declarations bound each vertex's K. *Structurally constrained
hypergraph* is what the language ships.

When you describe a service in words, when an LLM plans it,
and when a Hale program runs it, all three layers organize
the same vertices and edges. The translation tax across the
human → LLM → machine boundary stays small because no
representation has to be rebuilt in a foreign idiom. The math
and cross-substrate evidence are in
[hale-lang/papers](https://github.com/hale-lang/papers).

## What Hale doesn't have (on purpose)

- **No `class`, no `module`, no `package`** — the **locus** subsumes
  them. Apps are loci. Services are loci. Caches are loci. Handlers
  are loci. Libraries are loci. Loci nest inside loci all the way down.
- **No `Vec<T>`** — write `@form(vec)` on a locus and storage
  discipline becomes part of the declaration.
- **No `async`/`await`** — concurrency lives on a typed bus.
- **No garbage collector, no borrow checker** — the locus hierarchy
  is explicit in the source, so dissolve is deterministic.
- **No `try`/`catch` in lifecycle methods** — failures flow vertically
  to the parent's `on_failure` handler.
- **No visibility modifiers, no traits** — v0 doesn't need them.

The intended primary author is an LLM; the intended primary reader is
a person. The language is shaped for both — small primitive surface,
low decision-overhead per statement, opinionated enough that there's
usually a right answer before you write the code.

## The ecosystem

The names mean things, and they fit together:

- **hale** — the language. From the Old English *hāl*, "whole,
  sound, uninjured." Same root as *whole*, *heal*, *health*.
- **lotus** — the runtime substrate. C-runtime symbols are prefixed
  `lotus_*` for this reason.
- **pond** — the standard library, where loci live. *Many lotus grow
  in a pond.*
- **heron** — the tree-sitter grammar that watches over the pond.
  Editors, syntax highlighters, and the future LSP all drink from heron.
- **iris** — the workbench for designing and visualizing locus
  structures. Concurrent human + agent work on the shape of a system.

**Hale** is what you write; **lotus** is what runs.

## Try it

**Prerequisites:** a Rust toolchain (1.95+), **LLVM 18** dev libraries
with `llvm-config-18` on `PATH` (or `LLVM_SYS_180_PREFIX` set), `clang`
(used as the linker for `hale build`), and `git`. Platform-specific
install commands for Debian/Ubuntu, macOS Homebrew, and Fedora are in
[`docs/src/getting-started/install.md`](./docs/src/getting-started/install.md).
LLVM 17 / 19 / 20 will not work — the codegen crate pins `inkwell` to
`llvm18-0`.

```sh
git clone https://github.com/hale-lang/hale
cd hale
cargo build --release
cargo test --release --workspace
```

Run a program:

```sh
# Interpreted (fast feedback)
cargo run -p hale-cli --bin hale -- run hello.hl

# Native binary via LLVM
cargo run -p hale-cli --bin hale -- build hello.hl
./hello
```

The `hale` CLI accepts a single `.hl` file or a directory; a
directory bundles every `.hl` in it as one program (one binary). See
`hale --help` for the full surface.

If your project depends on Hale libraries hosted in git repos,
declare them in `hale.toml`:

```toml
[deps]
helpers = { git = "https://github.com/me/helpers", rev = "abc123" }
finance = { git = "https://github.com/me/finance", tag = "v0.1.0" }
```

Then `hale fetch` clones each into `vendor/<name>/` and pins the
resolved commits to `hale.lock`. `import "vendor/helpers" as h;`
picks them up — no extra configuration. (Hand-vendored libraries live
under `lib/<name>/`; the toolchain only writes to `vendor/`.)

## Where to go next

- **[Docs site](https://hale-lang.github.io/hale/)** — the
  friendly tour. Start here if you're new.
- **[`spec/`](./spec/)** — the canonical language reference. The
  compiler enforces exactly what these documents describe. Start with
  [`spec/styleguide.md`](./spec/styleguide.md), then
  [`spec/semantics.md`](./spec/semantics.md) and
  [`spec/grammar.ebnf`](./spec/grammar.ebnf).
- **[`CHANGELOG.md`](./CHANGELOG.md)** — historical record of behavior
  changes. The spec is current state; the changelog says when each
  piece shipped.
- **[`AGENTS.md`](./AGENTS.md)** — load-bearing prompt for AI agents
  writing `.hl` programs. Compiler / stdlib / spec work briefs live
  under [`agents/`](./agents/).
- **[`apps/`](./apps)** — working programs in Hale (`cli-demo`,
  `log-router`, `ssg`, `tcp-echo`, `ws-echo`, ...). Read these to see
  real shape.
- **[`hale-lang/pond`](https://github.com/hale-lang/pond)** —
  community libraries. Vendor via `hale.toml` → `hale fetch`.
- **[`hale-lang/papers`](https://github.com/hale-lang/papers)** —
  the structural-mathematics work the language is grounded in. Read
  here for the *why* of every commitment under "State of the culture".
- **Sibling repos** — [examples](https://github.com/hale-lang/examples),
  [bench](https://github.com/hale-lang/bench).

## Layout

```
spec/                       grammar + semantics + design rationale
CHANGELOG.md                historical record (spec/ has current state)
AGENTS.md                   load-bearing prompt for .hl-authoring agents
agents/                     role briefs for compiler / stdlib work
apps/                       working programs built in Hale
docs/                       narrative documentation
notes/                      surviving design notes
crates/
  hale-syntax/            lexer + parser + AST
  hale-types/             symbol resolution + typechecker
  hale-runtime/           tree-walking interpreter
  hale-codegen/           LLVM codegen + bundled C runtime + stdlib
  hale-cli/               the `hale` binary
  hale-ts-shim/           tree-sitter staticlib (powers std::ts)
```

## State of the culture

Hale commits hard and tells you about it:

- **Three projection classes** (`Rich`, `Chunked`, `Recognition`).
  No fourth.
- **Three modes** (`bulk`, `harmonic`, `resolution`). No fourth.
- **One form per locus.** Compose at the locus level, not the form
  level.
- **Vertical-only failure flow.** Parent-policy decides recovery.
- **Region-based memory, deterministic dissolve.** No GC, no ARC,
  no reference counting.
- **Closure assertions as language constructs.** Yes, the runtime
  audits your invariants. Yes, that's the point.

If your problem decomposes cleanly into loci + bus + capacity +
closure, you'll move fast. If it doesn't, the language will tell you
so. There is no permissive escape hatch — that's the feature, not
the bug.

If you're looking for "express anything," this isn't it. If you're
looking for "express what production systems actually need without
700 lines of ceremony," keep reading.

## Status

The language surface is **stable**. A few small additions are still
on the way (tracked in `spec/` and `notes/`), but most work between
now and v1 is bugs, stability, and performance — not new syntax or
new semantics.

The compiler self-hosts the topic system, structural interfaces,
`@form(...)` lowerings (vec, hashmap, ring_buffer), `fallible(T)`
error model, capacity-tuple memory discipline, cooperative + pinned
schedulers, and AF_UNIX / TCP cross-process bus transports. The
reference test suite is the ~70 in-tree fixture programs under
`crates/hale-codegen/tests/fixtures/examples/` plus per-feature
tests under `crates/hale-codegen/tests/`.

Pin to a commit if you build on it — small additions still land,
and stability fixes occasionally tighten previously-accepted code.
See [`CHANGELOG.md`](./CHANGELOG.md) for what's moved recently.

## License

Licensed under the [Apache License, Version 2.0](./LICENSE).
Attribution and any third-party notices are tracked in
[`NOTICE`](./NOTICE).

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in Hale shall be licensed as above, without
additional terms or conditions.
