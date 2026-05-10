# Codebase-onboarder — progress report

> Date: 2026-05-10. Snapshot after the m96+m97+m100+m102 +
> std::lang session. Plan: `notes/codebase-onboarding-design.md`.
> Validation: `notes/three-tower-validation.md`.

## The pitch (one paragraph)

A dev with an existing codebase points the tool at it. They see
their own code rendered as **three lotus towers** (operational,
import-graph, domain) and recognize the shape — *before* they
have learned what an Aperio lotus is. Aperio then absorbs the
codebase into Aperio source. The blind-migration framing: three
perspectives are the minimum count for the recognition to
operationalize without prior model.

Per the design doc, this is the **primary product target**,
ahead of the runtime-IDE arm.

## Where we are: data layer complete, rendering deferred

### ✅ Shipped — substrate

| What                              | Milestone | Status |
|-----------------------------------|-----------|--------|
| Tree-sitter substrate             | m96       | done   |
| Go grammar bundled                | m96       | done   |
| Language-agnostic facade (`std::lang::Lang`) | bonus | done   |

`std::lang::Lang` was not in the original plan — it landed in
response to the *"are we shaping it language agnostic"* check-
in. All Go-specific kind strings + idiom heuristics live behind
one type with a flavor switch. Adding Rust / Python / TS is a
flavor-arm extension, not an extractor rewrite.

### ✅ Shipped — three towers (data layer)

| Tower         | Mode       | Milestone | Status                |
|---------------|------------|-----------|-----------------------|
| Import-graph  | harmonic   | m97       | done (file-level)     |
| Operational   | resolution | m100 v0   | done (tree-sitter only; LSP deferred) |
| Domain        | bulk       | m102 v0   | done (15-entry Go lookup seed) |

All three emit JSON tower data. Per
`notes/three-tower-validation.md`, run on a shared Go fixture,
the three perspectives **triangulate** — the recognition lands
at the data level. File-level granularity has known awkwardness
(Go packages span files; rolled up downstream is the planned
fix), but the lotus shape is visible across the three views.

### ✅ Shipped — apps and tests

| App                       | What it does                              | Tests |
|---------------------------|-------------------------------------------|-------|
| `apps/ts-walk-demo`       | Substrate-validation walk of Go AST       | 4     |
| `apps/import-graph`       | Harmonic tower extractor                  | 5     |
| `apps/operational-graph`  | Resolution tower extractor                | 6     |
| `apps/domain-graph`       | Bulk tower extractor + morpheme rewriter  | 5     |

Each app is locus-shaped per the apps-are-loci ethos. The
`MorphemeRewriterL` inside domain-graph also validates the
**namespace-lotus pattern** — empty `params { }`, only methods,
self-method calls compose.

### ⏳ Not yet — rendering and follow-ons

| What                              | Milestone | Effort       | Status |
|-----------------------------------|-----------|--------------|--------|
| Tower aggregator (file → package) | m102.5    | small        | next critical-path |
| Cross-tower join layer            | m103a     | small-medium | needed for unified product |
| Graphics substrate (Bevy host)    | m98       | multi-month  | deferred until aggregator + join validated |
| Single-tower viz v0               | m99       | ~2 weeks     | blocked on m98 |
| Three-tower rendering + mode switch | m103   | ~2 weeks     | blocked on m99 |
| `std::lsp` (LSP-client)           | m101      | ~1-2 weeks   | deferred; tree-sitter heuristics suffice for v0 |
| Domain lookup expansion           | m102.5    | curation     | small but high-leverage |
| Transpiler v0 (Go → Aperio)       | m105      | ~3 weeks     | needs aggregator + join |
| `std::ui` (egui host)             | m104      | multi-month  | deferred |
| `std::mcp` (MCP server)           | m106      | multi-month  | deferred |
| Rust/Python/TS via Lang flavor arms | parallel | ~half-day each | deferred until Go validated |

## Distance to the demo product

Per the design doc, the **demo-product candidate** is m96 + m97
+ m98 + m99 — single-tower (import-graph, the cheapest)
rendered with the graphics substrate. We're 2-of-4 done on
that bundle. **The data layer is fully shipped; the gap is
purely visualization.**

The **real product** is m103 — three towers with mode switching.
We're 3-of-N done on that, where N includes m98+m99+m103.

## Critical path before any visualization investment

Per `notes/three-tower-validation.md`, three things to ship
before m98 starts:

1. **Tower aggregator** — roll file-level entries up into
   package-level loci. Pure Aperio source over emitted JSON.
   ~1 day.
2. **Cross-tower join layer** — link goroutine call-sites to
   target fn decls; HTTP handlers to their `HandleFunc` wiring
   (for subject derivation); motion-forms to operational loci.
   The composition that turns three independent JSONs into one
   unified tower model. ~3-5 days.
3. **Domain lookup table expansion** — current unknown rate is
   ~50% on the micro-fixture (`Request`, `Session`, `Audit`
   morphemes mark unknown). Curation pass against a top-100 Go
   projects vocabulary should drop that to <30%. ~half-day of
   curation work, no code.

None need new substrate. Pure Aperio composition over what's
shipped.

## Friction logged this session

Driving the next round of language follow-ups:

- **Lifecycle method bodies don't accept `return`** — `birth()`
  / `run()` / `dissolve()` reject `return`; short-circuit
  paths must factor into a free helper fn. m82 follow-up.
- **fn-pointer callbacks can't share state** — already in the
  brief, hit again here when the tagged-accumulator section
  formatter needs flavor context.
- **`aperio run` rejects qualified-name literals** — every
  app must be `aperio build`'d, not `run`. Pre-existing,
  consistent.
- **`list_dir` is newline-string, not `[String]`** — every
  extractor manually splits. Waits on `List<T>`.
- **`fs::mkdir` missing** — not hit yet but tracked for when
  a tower writer wants to mkdir its output dir.
- **Aperio doesn't have multi-file modules yet** — every app
  is one big main.ap. Means duplication where shared types
  would be natural. Tracked as Phase 1+ language work.

## Verdict

**Data layer is solid. The blind-migration framing holds on
real Go code.** Three towers, three perspectives, hand-validated
to triangulate. The architecture is now language-agnostic via
`std::lang`; adding a second source language is a half-day move
when justified.

The next bite-sized increment is the **tower aggregator**
(m102.5) — small, unblocks the cross-tower join, lets us run
the full pipeline against richer fixtures. Then graphics
(m98), then we have the demo product.

The runtime-IDE arm
(`notes/aperio-ide-design.md`) remains secondary; substrate
shared with this product (graphics, UI, MCP) gets built once
and used by both.
