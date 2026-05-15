# Perspective & observation

> **α** — How does a locus expose a serializable view of itself
> across process boundaries?

Covers:

- The `perspective` declaration: a typed, serializable
  parameter bundle that travels across the bus while the
  producing and consuming binaries compile from the same source
  (so the schema is the type).
- **`stable_when`** predicates: the runtime asks "is this
  perspective ready to ship?" before publishing.
- **Hot-load**: how a consumer atomically swaps an active
  perspective without a torn read.
- The relationship between perspective and the contract /
  projection-class surface — perspective is *one* projection
  axis among many.
- Cross-depth perspective as a structural primitive: an
  observer's depth in the locus tower is itself a projection
  axis that determines whether an interaction reads as
  coordination or as one-sided constraint.

*This chapter is under construction. The
[`spec/types.md`](https://github.com/aperio-lang/aperio/blob/main/spec/types.md)
"Perspective types" section and `spec/runtime.md` "Perspective
infrastructure" are the canonical references in the meantime.*
