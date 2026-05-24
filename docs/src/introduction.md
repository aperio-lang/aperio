# Introduction

*hypergraph programming*

A language where the shape of your code matches the shape of
your thinking.

---

You know that feeling when you describe a system out loud —
*"a matchmaker holds a queue of waiting players, spawns a
match when enough are queued, then goes back to waiting"* —
and then the code you actually have to write bears no
resemblance to those words?

Mutexes appear. Async machinery. Lifecycle wiring. Five
files. By the time it's working, the sentence you started
with is buried under the scaffolding.

Hale is a bet that the gap between *how you describe a
system* and *what you type into the editor* doesn't have to
be there.

## A matchmaker, in Hale

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

Every phrase from the description has a syntactic home, in
roughly the order you thought about them:

- *"a service"* → `locus Matchmaker`
- *"a queue of waiting players"* → `capacity { heap waiting of Player; }`
  (the `@form(vec)` annotation gives it `push`, `get`, `len`, and friends)
- *"receives players wanting matches"* → `subscribe JoinQueue as on_join`
- *"announces matches"* → `publish MatchReady`
- *"when enough are queued"* → the `if` inside `on_join`

That's the whole service. No mutex selection, no channel
types to pick, no async/await ceremony, no explicit
lifecycle wiring, no error-handling at every boundary. Each
of those choices is something Hale commits to at the
structural layer, so you don't repeat them at every
callsite.

`@form(vec)` is itself a real decision — not arbitrary.
`@form(ring_buffer)` would give the same shape with a bounded
capacity and drop-on-full; `@form(hashmap)` keyed by player
id would give you natural ID-based cancellation. Forms are
how Hale exposes those choices — we cover them in
**Concepts**.

## The ecosystem

The names mean things, and they fit together:

- **hale** — the language. From the Old English *hāl*, "whole,
  sound, uninjured." Same root as *whole*, *heal*, *health*.
- **lotus** — the runtime substrate the language sits on top
  of. C-runtime symbols are prefixed `lotus_*` for this reason.
- **pond** — the standard library, where loci live. *Many
  lotus grow in a pond.*
- **heron** — the tree-sitter grammar that watches over the
  pond. Editors, syntax highlighters, and the future LSP all
  drink from heron.
- **iris** — the workbench for designing and visualizing
  locus structures. Concurrent human + agent work on the
  shape of a system.

Two names, two layers, one project. **Hale** is what you
write; **lotus** is what runs.

## See it on your own code

The matchmaker above is constructed. The real test of
whether Hale's structural model fits the way you think is
on code you already have.

In whatever LLM-coding tool you use ([Claude
Code](https://claude.ai/code), Cursor, anything else), drop
this project's
[`AGENTS.md`](https://github.com/hale-lang/hale/blob/main/AGENTS.md)
into the agent's context, then ask it to re-read a module
or service from your existing codebase **in terms of loci,
contracts, and bus topics**.

What usually comes back is a structural decomposition that
matches your mental model of the system with surprising
accuracy — because the agent is using the same recursive
locus vocabulary you already use when reasoning about your
code. The friction you normally feel between *how you think
about this system* and *what's literally on the page*
largely goes away.

If the decomposition looks wrong or unhelpful, the thesis
fails for your codebase — open an issue, that's useful
feedback. If it looks right, you've felt the structural fit
from the reading side without writing a line of Hale.

## Three substrates, one shape

There's a reason the matchmaker decomposes the same way on
paper, in a Hale program, and inside an LLM's planning head.

When K things attach to one coordination point, the working
state needed to hold them together costs K log₂ K bits. That
bound shows up everywhere coordination happens — across human
working memory (Miller's 7 ± 2, Cowan's 4 ± 1), spans of
control (5–9), surgical teams (5–10), mixture-of-experts
active experts (1–8), and multi-agent LLM orchestration
(saturating at 4–8). The same ceiling, k̄ ∈ [4, 10],
substrate-invariant. The math and the cross-substrate evidence
are in
[hale-lang/papers](https://github.com/hale-lang/papers).

A Hale program is a *structurally constrained hypergraph*:
loci are the vertices, topics are hyperedges binding
publishers and subscribers, and capacity declarations bound
each vertex's K. The paper calls this shape a "coordination
locus — a structurally-local point at which attachments are
made." Hale's syntax is that definition turned into a
declaration site.

This is why the translation tax across the
human → LLM → machine boundary stays small. When you describe
a service in words, you're already shaping it as loci with
bounded attachment. When an LLM plans it, its hidden state
organizes the same K-bounded structure (substrate-invariance
isn't optional for the LLM either). When a Hale program runs,
the compiler enforces that structure literally. Each step
uses the same vertices and edges; no representation has to
be rebuilt in a foreign idiom.

Most languages charge the tax at every boundary: you describe
a service one way, an LLM rewrites it into language idioms,
you re-rewrite it into mutexes, channels, and lifecycle
wiring. Structural information leaks at each step. Hale keeps
the graph intact all the way down.

## Status and shape

The language surface is **stable**. A few small additions are
still planned, but most work between now and v1 is bugs,
stability, and performance — not new syntax or new semantics.
Pin to a commit if you build on it; small additions still
land, and stability fixes occasionally tighten previously-
accepted code.

The compiler ships native codegen via LLVM 18 plus a
tree-walking interpreter for fast feedback. The standard
library (`std::io::tcp`, `std::io::fs`, `std::http`,
`std::time`, `std::str`, and more) is bundled into every
program.

Head to **Getting Started** to install Hale and write your
first locus. Once you've felt the shape, the **Concepts**
chapters walk through the structural model in depth — the
locus, the bus, capacity, lifecycle, modes. **Reference** is
the canonical spec.

---

> **A note on what else this might be.** The structural model
> Hale is built on isn't, in principle, software-specific.
> The same recursive hypergraph organizes coordination across
> institutions, biological regulatory networks, physical
> systems. Hale's surface could, eventually, become a
> *design language* for any substrate where capacity is
> allocated and flow is hierarchical. We hold that lightly —
> the immediate work is the programming language. The deeper
> story lives in
> [hale-lang/papers](https://github.com/hale-lang/papers).
