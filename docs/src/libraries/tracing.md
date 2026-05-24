<!-- Synced from hale-lang/pond/tracing/README.md by tools/sync-pond-docs.sh — do not edit here. -->

# pond/tracing — span tree mirroring the locus tower

Suggested import alias: **`trace`**

```hale
import "vendor/pond/tracing" as trace;
```

One Tracer per app; spans nest naturally with locus instantiation
(the parent span is the enclosing span). End-of-span fires the
`SpanCompleted` topic; sinks subscribe and project — print as a
tree, batch and export to OTLP, fan out to multiple backends, etc.

## Surface

```hale
type SpanId { id: String; }
type Span   { id: SpanId; parent: SpanId; name: String;
              start_ns: Int; end_ns: Int; attrs: String; }

locus Tracer {
    params { service_name: String = "pond-tracing"; }
    fn start_span(name: String, parent: SpanId) -> SpanId;
    fn end_span(id: SpanId);
    fn add_attr(id: SpanId, key: String, val: String);
    fn export_otlp(endpoint: String);            // see FRICTION.md
    fn last_error_kind_str()   -> String;
    fn last_error_detail_str() -> String;
    fn last_export_body_str()  -> String;
}

// Topic — wire subject "trace.span.completed". See FRICTION.md
// for why this lib ships the wire subject directly rather than a
// `topic` decl.
//
//   bus { subscribe "trace.span.completed" as on_span
//                                          of type trace::Span; }
```

## Start + end usage

```hale
import "vendor/pond/tracing" as trace;

locus App {
    run() {
        let tr = trace::Tracer { service_name: "my-service" };

        // Root span — parent.id == "" flags it as a root.
        let outer = tr.start_span("request.handle",
                                  trace::SpanId { id: "" });
        tr.add_attr(outer, "http.method", "GET");

        // Nested span — pass `outer` as the parent.
        let inner = tr.start_span("db.query", outer);
        tr.add_attr(inner, "db.statement", "SELECT * FROM widgets");
        tr.end_span(inner);

        tr.end_span(outer);
    }
}
```

The recursive-locus principle says spans naturally nest with
locus instantiation — every span declares the SpanId of the
enclosing scope. v1 requires the user to thread `parent`
explicitly through `start_span`; a future runtime-injection pass
(see `FRICTION.md`) would walk the locus tower at birth /
dissolve and emit start / end_span automatically, dropping the
explicit-parent argument.

## SpanCompleted subscriber

`Tracer.end_span(id)` finalizes the span, publishes a `Span`
record on the wire subject `"trace.span.completed"`, and stashes
the row in an internal export buffer. Downstream subscribers wire
up exactly like any other typed bus subscriber:

```hale
import "vendor/pond/tracing" as trace;

locus TracePrinter {
    bus {
        subscribe "trace.span.completed" as on_span of type trace::Span;
    }

    fn on_span(s: trace::Span) {
        let dur_ns = s.end_ns - s.start_ns;
        println(s.name, "  parent=", s.parent.id,
                "  dur=", to_string(dur_ns), "ns");
    }
}

fn main() {
    // Bus ordering rule (AGENTS.md): subscriber FIRST.
    TracePrinter { };
    // Then the Tracer + producer.
    let tr = trace::Tracer { service_name: "demo" };
    // ... start_span / end_span calls ...
}
```

Multiple subscribers are fine — each gets its own copy of every
`Span`. A JSON-line logger, an OTLP exporter, and an in-process
tree printer can coexist on the same Tracer.

## Files

| File | What it holds |
|------|---------------|
| `types.hl`  | `SpanId` + `Span` shape records. |
| `tracer.hl` | `Tracer` locus + `__build_otlp_json` / `__nth_field` / `__remove_row` / `__duration_to_int` helpers. |
| `examples/trace-tree/main.hl` | Demo: outer span + nested inner span + tree printer subscriber. |

The contract calls for a separate `topic SpanCompleted { ... }`
decl; this lib ships the wire subject directly and skips the
topic decl due to a compiler-side import-mangling gap. See
`FRICTION.md`.

## Catalog placement

- `SpanId`, `Span` — pattern 5 (shape type).
- `Tracer` — pattern 3 (service locus, bus publisher, long-lived
  state-bearing).
- `__build_otlp_json`, `__nth_field`, `__remove_row`,
  `__duration_to_int` — pattern 6 (free fn). Not promoted to a
  namespace lotus because each addresses a different concern;
  three of the four are flagged in `FRICTION.md` as
  duplicate-suspected candidates for lifting once a pond utility
  seed exists.

## Verification

```bash
hale build \
    pond/tracing/
```

The library type-checks cleanly. The bare-lib build fails at
codegen with "program has no `fn main()`" (same outcome
`pond/subprocess/`, `pond/http/client/`, `pond/math/stats/` etc.
see — Hale's codegen-v0 doesn't ship a `--lib` mode). The
end-to-end verification path is the example build:

```bash
hale build \
    pond/tracing/examples/trace-tree/

pond/tracing/examples/trace-tree/trace-tree
```

Expected output:

```
--- trace tree (2 spans) ---
* request.handle (<dur>ns) id=1
  * db.query (<dur>ns) id=2
trace-tree: OK
```

Exit code 0; a non-zero exit means a structural failure (count
mismatch, sub-span never observed by the sink, etc.) and the
demo prints a `FAIL:` line before exiting.

## Cross-cutting

- Span timestamps use `std::time::monotonic()` per the assignment.
  Duration is stored as `Int` ns in the public `Span` record (the
  contract type); the lib bridges the Duration → Int gap via
  `__duration_to_int` (string round-trip — see `FRICTION.md`).
- Two-channel rule: every Tracer method is infallible. The
  contract's `export_otlp(endpoint) -> () fallible(IoError)`
  deviation (locus methods can't declare `fallible`) is logged
  in `FRICTION.md` with the same "deviate to `last_error_kind` +
  `last_error_detail`" pattern `pond/http/client/Client` uses.
- Six-pattern catalog: the entire lib stays inside patterns 3 / 5
  / 6. No invented categories.
