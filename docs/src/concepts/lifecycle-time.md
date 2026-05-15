# Lifecycle & time

> **α** — How does a locus come into being, run, and dissolve?
> And what does "concurrent" mean here?

Covers:

- **The five lifecycle methods**: `birth`, `accept`, `run`,
  `drain`, `dissolve`. When each fires; what the runtime
  guarantees about ordering.
- **Schedule classes**: `cooperative` (default; shares a
  scheduler thread, yields at substrate-cell boundaries) and
  `pinned` (owns its own OS thread, optionally CPU-affinitized).
  Why there's no third option.
- **Drain cascade**: how SIGINT becomes a depth-first
  drain-then-dissolve of the whole locus tree, with bus events
  still processed during shutdown.
- **Dissolve timing**: statement-position vs. let-bound vs.
  long-lived; deferred-dissolve at scope exit.
- Why Aperio has no `async` / `await` — concurrency lives in
  the lifecycle state machine + cooperative yields + the bus.

*This chapter is under construction. The
[`spec/runtime.md`](https://github.com/aperio-lang/aperio/blob/main/spec/runtime.md)
and [`spec/semantics.md`](https://github.com/aperio-lang/aperio/blob/main/spec/semantics.md)
"Dissolve timing rules" section are the canonical references in
the meantime.*
