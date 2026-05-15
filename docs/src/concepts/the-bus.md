# The bus

> **α** — How do loci communicate without referring to each
> other by name?

Builds on [Recursive composition](./recursive-composition.md).
Covers:

- **Topics** as typed, first-class channels. The payload type
  travels with the declaration, not with each subscriber.
- `subscribe`, `publish`, and the `<-` send operator.
- Why siblings communicate through the bus (and through the
  parent's mediation) rather than holding direct references.
- **Bindings**: how a topic that's purely in-process by default
  can be wired to AF_UNIX, TCP, or NATS at deployment time —
  with no code change to the publishing or subscribing loci.
- The closed-world optimization: when a topic is used only
  inside one locus, the compiler elides the bus entirely.

*This chapter is under construction. The
[`spec/semantics.md`](https://github.com/aperio-lang/aperio/blob/main/spec/semantics.md)
"Topic declarations" section is the canonical reference in the
meantime.*
