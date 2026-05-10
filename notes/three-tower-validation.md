# Three-tower hand-validation — m96 + m97 + m100 + m102

> Date: 2026-05-10. Run after all three extractors landed
> (m96 substrate, m97 harmonic, m100 v0 resolution, m102 v0
> bulk). The point of the exercise is to gut-check whether the
> three perspectives triangulate the lotus shape on real
> foreign code **before** investing in any visualization
> substrate (m98+).

## Method

A single Go fixture (`apps/operational-graph/fixture/`,
4 files, ~80 LOC) was processed by all three extractors. Output
is JSON tower data per perspective. No rendering — pure data
inspection.

The fixture mirrors a small HTTP service:

- `main.go` — entry point, `init()` + `main()` + spawns a
  background worker
- `handlers.go` — two `func(http.ResponseWriter, *http.Request)`
  handlers
- `worker.go` — `for { select { } }` infinite loop, named +
  anonymous goroutines
- `store.go` — three type declarations (`RequestCache`,
  `SessionManager`, `AuditLogger`)

## Tower outputs

### Harmonic (import-graph — m97)

```json
{
  "files": [
    {"id": "worker.go",   "imports": ["log", "time"]},
    {"id": "handlers.go", "imports": ["fmt", "net/http"]},
    {"id": "main.go",     "imports": ["log", "net/http"]},
    {"id": "store.go",    "imports": ["sync"]}
  ]
}
```

What the harmonic view shows: structural relationships across
files. `net/http` appears in `main.go` + `handlers.go` (the
two HTTP-aware files); `log` in `main.go` + `worker.go`
(diagnostic-emitting); `sync` only in `store.go`
(state-protection). The dependency shape is
self-explanatory from this data alone.

### Resolution (operational — m100)

```json
{
  "operational": {
    "main":       [{"file": "main.go"}],
    "init":       [{"file": "main.go"}],
    "handlers":   [{"file": "handlers.go", "name": "helloHandler"},
                   {"file": "handlers.go", "name": "statusHandler"}],
    "goroutines": [{"file": "worker.go", "kind": "named"},
                   {"file": "worker.go", "kind": "anonymous"},
                   {"file": "main.go",   "kind": "named"}],
    "long_loops": [{"file": "worker.go"}]
  }
}
```

What the resolution view shows: the runtime / process model.
`main` + `init` in main.go (root locus' lifecycle hooks);
two handlers (bus subscribers); three goroutines (child
loci); one long-running loop (a `run()` body equivalent).

### Bulk (domain — m102)

```json
{
  "domain": {
    "types": [
      {"name": "RequestCache",   "motion": "<unknown:Request>-remembering"},
      {"name": "SessionManager", "motion": "<unknown:Session>-managing"},
      {"name": "AuditLogger",    "motion": "<unknown:Audit>-logging"}
    ]
  }
}
```

What the bulk view shows: motion-form vocabulary for the
codebase. Three nouns map to motions: remembering (Cache),
managing (Manager), logging (suffix rule on Logger).
Three morphemes (`Request`, `Session`, `Audit`) honestly
mark unknown — the lookup table doesn't cover them.

## Does this triangulate?

**Yes — with caveats.** Reading all three towers side-by-side
on this small fixture, the lotus shape lands:

- **One concrete locus emerges per perspective:** the running
  service. main.go owns the root locus (resolution view);
  imports show the dependency skeleton (harmonic); the type
  declarations name what the service operates on (bulk).
- **Cross-tower coherence is visible.** `worker.go` shows up
  in resolution (long_loops, goroutines), in harmonic (log +
  time imports for a tick-driven worker), and **doesn't**
  show in bulk (no type declarations there) — which itself is
  informative. A file's *absence* from one tower carries
  signal.
- **Honest unknowns preserve the recognition.** `Request`,
  `Session`, and `Audit` mark explicit unknown markers
  rather than fabricating motion-forms. A user can
  immediately tell which morphemes are mapped from real
  rules vs. flagged as gaps. **This is the right design call.**

The three perspectives feel like three lenses on the same
artifact, not three unrelated graphs of three different
artifacts. The blind-migration framing holds: a developer
shown these three towers without prior Aperio knowledge could
reasonably triangulate "this code has the shape the language
is talking about."

## Friction surfaced

Things that need addressing before the visualization layer (m98+)
or the three-tower demo target (m103) ships:

### 1. v0 file-level granularity is awkward for Go

In Go, **the package** (a directory) is the natural unit of
encapsulation, not the file. `main.go` + `handlers.go` +
`worker.go` + `store.go` together form one `package main`.
The harmonic tower shows them as four separate entries; the
resolution tower scatters lifecycle markers across them; the
bulk tower picks types from one file in isolation.

Demo recognition would be sharper if the towers rolled up by
package:

```
package "main" (resolution): root_locus { main, init, 2 handlers, 3 goroutines, 1 long_loop }
package "main" (harmonic):   imports = ⋃(per-file imports) = {log, time, fmt, net/http, sync}
package "main" (bulk):       types = [RequestCache, SessionManager, AuditLogger]
```

That's a downstream pass over the file-level data the v0
extractors emit — a separate "tower aggregator" stage.
Tracked as a follow-up; v0 file-level shape preserves enough
information for that aggregation to happen.

### 2. Domain extractor needs more morphemes

Three out of three top-level type morphemes
(`Request`, `Session`, `Audit`) hit the unknown branch on
this fixture. The lookup table is too thin for real Go code.
Per the design doc, the table is *per-language* extensible;
a curation pass over a corpus of common Go projects would
expand it. **Current threshold for ship readiness:** unknown
rate < 30% on a top-100-Go-projects corpus. Currently ~50% on
this micro-fixture; needs work.

Or: ship a "fallback motion: lowercase the morpheme + 'ing'
when the morpheme is itself a verb" rule. But detecting
"is-a-verb" without a dictionary is fragile; the unknown
marker is more honest than a guess.

### 3. Goroutine destination opaque

The resolution tower says "3 goroutines" but doesn't say
which loci they should map to. `go backgroundWorker()` clearly
spawns a child locus named `backgrounding` (working from the
domain rewriter), but the operational extractor at v0 only
records call-site existence, not call-target. Linking
goroutine call sites to their target functions is m100.5 work
(needs LSP, or fallback resolution from `function_declaration`
names in the same package).

### 4. Handler subjects are unspecified

The resolution tower says "two handlers" but doesn't say what
HTTP path each subscribes to. `mux.HandleFunc("/hello",
helloHandler)` is the wiring; the extractor would need to
follow that call to know `helloHandler`'s subject is
`http.GET./hello`. Cross-call analysis again — m100.5 LSP
territory, or local data-flow analysis as a fallback.

### 5. No cross-tower joins yet

Each tower is independent JSON. To render the unified lotus
view (m103 product), a downstream pass needs to:

- Roll file-level entries up into package-level loci.
- Join goroutine call-sites to their target fn declarations.
- Join handler decls to their wiring (HandleFunc) for subject
  derivation.
- Annotate each locus with its motion-form (cross-link
  resolution + bulk towers).

This join step is itself a milestone (m102.5 or m103a). It's
**not** part of any individual extractor — it's the
composition that turns three independent JSON files into one
unified tower model.

## Verdict

**Three towers as data is the right substrate to commit to
m98 graphics on.** The recognition lands; the friction is
identifiable and bounded; the next steps are well-scoped.

Critical follow-ups before m98 or m103:

1. Tower aggregator (file → package).
2. Domain lookup table expansion (top-100 Go projects pass).
3. Cross-tower join layer.
4. (Optional) goroutine call-site → target resolution.

None of these need new substrate. They're all Aperio source
composing the shipped extractors.

## What we do NOT need to commit to yet

- Bevy / 3D graphics. The textual JSON towers already
  triangulate; 2D HTML/SVG would be sufficient for the demo.
  Graphics is a UX investment, not a thesis investment.
- LSP integration. Tree-sitter heuristics get us 80% on the
  cases that matter; LSP is the polish layer, not the
  enabling layer.
- Per-language extractors beyond Go. Validate the demo
  product on Go alone first.
