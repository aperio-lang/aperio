# `std::log`

Structured logging on the bus. Phase 6 m95 ships three pieces:

- **`std::log::LogEvent`** — the typed payload.
- **`std::log::Logger`** — a publishing locus with a cascading
  namespace.
- **`std::log::StdoutSink`** — a default sink that subscribes
  on `log.**` and prints `[LEVEL path] msg` to stdout.

The Aperio-shape: every log event is a typed payload published
on a hierarchical subject. Sinks are bus subscribers — same
mechanism as any other Aperio bus user. Cross-process tailing
comes for free once a subject is bridged via deployment config
(no runtime changes needed; the m72 TCP framing already exists).

This namespace composes on m94 (subject wildcards): the
publish-side wildcard authorization (`publish "log.**"`) is
what lets `Logger` send on a runtime-computed subject, and the
subscribe-side wildcard match (`subscribe "log.**"`) is what
lets `StdoutSink` see every Logger's events from one
declaration.

## Types

### `std::log::LogEvent`

```aperio
type LogEvent {
    level: Int;     // 1=INFO, 2=WARN, 3=ERROR, 4=DEBUG, 5=TRACE
    msg:   String;  // free-form message
    path:  String;  // publishing logger's full_path, e.g. "app.db"
}
```

The payload type for every Logger publish. `path` is included
so a sink subscribing to a single subject can still display the
publisher's hierarchy without inspecting the subject itself.

Levels are Int constants pending enum-variant pattern support
(per the language roadmap). When variant patterns land, this
becomes a sum type and sinks can `match level` cleanly.

## Loci

### `std::log::Logger`

A publishing locus with cascading namespace.

#### Synopsis

```aperio
locus Logger {
    params {
        name:        String = "root";
        parent_path: String = "";
    }
    fn info(msg: String);
    fn warn(msg: String);
    fn error(msg: String);
    fn debug(msg: String);
    fn trace(msg: String);
}
```

#### Semantics

- On birth, computes `full_path`:
  - If `parent_path` is empty: `full_path = name`.
  - Otherwise: `full_path = parent_path + "." + name`.
- Each method publishes a `LogEvent` on the subject
  `"log." + full_path`.
- The locus declares `publish "log.**" of type LogEvent`,
  which (m94) authorizes the runtime-computed subject.
- Loggers do not chain via `accept` in v0 — pass `parent_path`
  explicitly when constructing a child Logger.

#### Examples

A flat application log:

```aperio
fn main() {
    std::log::StdoutSink { };
    let log = std::log::Logger { name: "app" };
    log.info("starting");
    log.warn("memory pressure");
    log.error("upstream timeout");
}
```

Output:

```
[INFO app] starting
[WARN app] memory pressure
[ERROR app] upstream timeout
```

Cascading namespaces — child Loggers nest under their parent's
path:

```aperio
fn main() {
    std::log::StdoutSink { };
    let app = std::log::Logger { name: "app" };
    let db  = std::log::Logger { name: "db", parent_path: "app" };
    let api = std::log::Logger { name: "api", parent_path: "app" };
    app.info("starting");
    db.info("connected");
    api.warn("slow");
    db.error("query failed");
}
```

Output:

```
[INFO app] starting
[INFO app.db] connected
[WARN app.api] slow
[ERROR app.db] query failed
```

Three-level nesting:

```aperio
fn main() {
    std::log::StdoutSink { };
    let q = std::log::Logger { name: "query", parent_path: "app.db" };
    q.info("running");   // [INFO app.db.query] running
}
```

### `std::log::StdoutSink`

Default sink. Subscribes to `log.**` and prints
`[LEVEL path] msg` per event.

#### Synopsis

```aperio
locus StdoutSink {
    bus { subscribe "log.**" as on_event of type LogEvent; }
}
```

#### Semantics

- `log.**` (zero+ trailing wildcard) matches every subject a
  `Logger` publishes on, regardless of namespace depth.
- All levels print to stdout in v0. WARN/ERROR routing to
  stderr is a follow-up — needs an `eprintln`-style primitive
  that doesn't yet exist.
- Multiple `StdoutSink` instances would each print every event
  (one line per sink); typically you instantiate exactly one in
  `main()`.

#### Ordering

Subscribers register at `birth()`. **Instantiate `StdoutSink`
(and any other log subscribers) before any `Logger`** so the
sinks are listening when Logger.info/warn/error fires. If a
Logger's `birth()` itself calls `info(...)` (uncommon but
legal), follow the same rule: subscribers first, publishers
last.

## Custom sinks

A user-defined sink is just a locus that subscribes to a `log`
pattern of choice. Patterns let a sink scope to a sub-tree:

```aperio
// Only see events from app.db and below.
locus DbOnlySinkL {
    bus {
        subscribe "log.app.db.**" as on_db of type std::log::LogEvent;
    }
    fn on_db(e: std::log::LogEvent) {
        // Custom rendering, file writing, network forwarding...
        println("[db] ", e.path, " ", e.msg);
    }
}
```

`log.app.db.**` matches `log.app.db` (the db logger's own
events) and any descendant — `log.app.db.query`,
`log.app.db.cache`, etc. It does not match `log.app` (parent)
or `log.app.api` (peer).

To see *every* event regardless of subject (e.g. a
forwarding sink), subscribe on `log.**`.

## Cross-process tailing

A `Logger` publishing on `log.app` and a tailer subscribing on
`log.**` will work cross-process once the subject is bridged
via the deployment-config TCP transport. Both sides use the
same source-level declaration; the bridge is a runtime concern,
not a source concern. The substrate (m72 length-prefix framing)
already exists. A source-level `std::bus::expose` API to
configure bridging from `.ap` source is a future milestone.

## Limitations (m95)

- **No `Logger.child(name)` method.** Child Loggers take an
  explicit `parent_path`. A method that returns a freshly-
  constructed child is blocked on the "function returns a
  locus" language paper-cut.
- **Levels are Int constants.** When enum-variant patterns
  ship, levels become a sum type and sinks can `match` on them.
- **All levels go to stdout.** No stderr routing for WARN/ERROR
  in v0.
- **No structured fields beyond `msg`.** Adding key-value pairs
  needs either generics (`Map<K,V>`) or a fixed-size array of
  `(String, String)` tuples; both are deferred. For v0 either
  pre-format into the `msg` string or build a custom event type
  on a custom subject.
- **No log-level filtering at the sink.** `StdoutSink` prints
  everything. A custom sink can filter with `if e.level >= 2`.
- **`println` from a logger handler shares stdout** with `log`
  output. Sinks that care about isolated streams should write
  to a file instead.

## See Also

- [Roadmap](./roadmap.md) — Phase 6 substrate plan.
- [What you can build today](./ready-today.md) — capability
  matrix.
- `crates/aperio-codegen/runtime/stdlib/log.ap` (in the
  language repo) — the implementation.
- `crates/aperio-codegen/tests/stdlib_log.rs` (in the language
  repo) — end-to-end tests covering levels, cascade, and
  subtree-pattern subscription.
