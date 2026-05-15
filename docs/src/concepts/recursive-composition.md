# Recursive composition

> **α** — How do loci nest inside loci, and why is flow
> vertical-only?

Builds on [The locus](./the-locus.md). Covers:

- Parent ↔ child relationships and how they're declared.
- The **contract surface** — what crosses the boundary
  between a parent and a child.
- *Vertical-only flow*: parents read children through the
  contract; children publish upward through the contract;
  siblings do *not* see each other directly.
- Why lateral sibling access is structurally forbidden and
  how to route coordination through the parent instead.
- A worked example: an app locus containing a service locus
  containing per-connection handler loci.

*This chapter is under construction. The
[`spec/design-rationale.md`](https://github.com/aperio-lang/aperio/blob/main/spec/design-rationale.md)
F.6 / F.11 / F.14 sections are the canonical references in the
meantime.*
