# MOA bus subject conventions

> Informative at v1; not enforced by the compiler. Apps that follow
> these conventions interoperate by construction — a generic
> visualizer can subscribe to any tower-emitting app; a generic
> replay tool can record any MOA app's runtime events.

The framework's typed lateral bus carries every cross-concern delta.
This document standardizes the *subject names* memory-owners
publish under, so that observers (debuggers, visualizers,
recorders, simulators) speak a vocabulary the publisher recognizes
without bilateral negotiation.

## The base shape

A subject is a dot-separated lowercase path. Convention:

```
<concern>.<shape>.<event>
```

- **concern** — what kind of state this is about (`source`,
  `scene`, `book`, `runtime`, `agent`, `control`, …). One per
  memory-owner family.
- **shape** — the structural piece of the state (`tower`, `node`,
  `frame`, `flower`, `pulse`, `message`, …).
- **event** — the change (`added`, `removed`, `updated`,
  `completed`, `fired`, …).

Examples:
- `source.tower.node.added` — a new node appeared in the source tower
- `scene.flower.removed` — a flower left the rendered scene
- `runtime.event.bus_send` — a bus send happened in the watched app
- `control.mode.changed` — the IDE's current mode flipped

## Standard delta-stream families

A memory-owner that *holds a tree* publishes a four-subject family:

```
<concern>.<shape>.added       — node attached to the tree
<concern>.<shape>.removed     — node detached from the tree
<concern>.<shape>.updated     — node fields changed in place
<concern>.<shape>.sweep.complete — batch boundary (commit marker)
```

The `sweep.complete` marker matters when one logical update produces
many add/remove/update deltas: subscribers can hold off projection
recompute until the batch closes.

A memory-owner that *holds a stream* publishes:

```
<concern>.<shape>.fired       — event occurred
<concern>.<shape>.completed   — event resolved (for events with duration)
```

A memory-owner that *holds a single value* publishes:

```
<concern>.<shape>.changed     — value rewritten
```

## m94 wildcard usage

The bus router supports trailing `**` wildcards (per `spec/runtime.md`).
Generic observers subscribe to `<concern>.**` to catch the whole
family:

```aperio
bus {
    /// ingest: save — buffers every runtime event for later replay
    subscribe "runtime.event.**" as on_runtime of type RuntimeEvent;
}
```

This is how a recorder catches every event in a watched app's runtime
without enumerating each subject by hand.

## Reserved top-level concerns

The following first-segment names are reserved for MOA substrate
roles; apps should not invent new meanings for them:

| Concern | Used for | Canonical payload |
|---|---|---|
| `runtime.event` | Observation envelope from a watched program's bus | `moa::RuntimeEvent` |
| `control` | Mode / playback / clock signals from a controlling tool | app-defined |
| `clock` | Tick / now signals from a clock substrate | `moa::Tick` |

App-specific concerns (`source`, `scene`, `book`, `agent`, `editor`,
`scenario`, etc.) are owned by the app and can be named freely;
the conventions in this document still apply to the *shape* of the
subject (added/removed/updated/sweep.complete for trees; fired/
completed for streams; changed for singles).

## One publisher per subject family

A rule, not just a convention: each subject family has **exactly one
canonical publisher** in any given app. Multiple loci publishing on
the same family makes the system unauditable — observers can't ask
"who is the source of truth for `<concern>.<shape>`."

If two memory-owners need to publish *related* deltas, give them
different family names (`book.snapshot.*` vs `book.delta`, both
canonical-published by `MdGatewayL` in market-book — same publisher,
two families).

## Future enforcement

At v1 this is a discipline-only document. v1.x may promote:

- **Subject-pattern types** — typecheck-time verification that a
  publisher's declared subject matches its actual payload type.
- **Family-level uniqueness** — compile-time rejection if two
  memory-owners publish overlapping subject patterns.
- **Wildcard subscription warnings** — flag a subscriber that takes
  `**` and doesn't classify ingest per sub-family.

None of these ships at v1; the conventions land as words first.

## Cross-references

- `MOA.md` — the architecture this serves
- `types.ap` — `RuntimeEvent`, `Tick`, the substrate payloads these
  conventions wrap
- `docs/src/moa/patterns/broadcast-snapshot.md` — the canonical
  application of these conventions: public delta + snapshot families
  with the many-to-one request channel asymmetry
- `docs/src/moa/patterns/private-streams.md` — the carve-out: when
  per-recipient subject suffixes are warranted
- `spec/runtime.md` — bus router, m94 wildcard semantics
- `spec/stdlib.md` — m94 stdlib bus changes
