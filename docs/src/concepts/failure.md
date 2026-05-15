# The two failure channels

> **α** — Why does Aperio have two separate failure mechanisms,
> and how do you choose between them?

Covers:

- The **closure-violation channel** (structural failure): a
  locus's declared invariant breaks; `on_failure` on the parent
  handles it; recovery primitives (`restart`, `quarantine`,
  `bubble`) decide what happens next. The `↑` mechanic.
- The **value-error channel** (`fallible(E)`): a call may
  fail and the caller MUST address it inline via an `or`
  clause (`or raise`, `or default`, `or handler(err)`).
- Why the two channels are orthogonal and meet only at the
  implicit-main-locus root boundary.
- **Where each channel is allowed to live**: `fallible(E)` on
  free functions and `@form(...)`-synthesized methods only;
  user-declared locus methods communicate failure structurally
  via the closure-violation channel.
- Why Aperio has no `panic` / `assert` / exception machinery.

*This chapter is under construction. The
[`spec/semantics.md`](https://github.com/aperio-lang/aperio/blob/main/spec/semantics.md)
"Fallible call semantics" section and `spec/design-rationale.md`
§F.9 are the canonical references in the meantime.*
