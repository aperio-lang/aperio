# Structured logging

Hale's logging surface is bus-shaped: every log event is a
typed payload published on a hierarchical subject. Sinks
subscribe to subjects and decide what to do with each event.
The same model that gives you decoupled pub/sub for
application messages gives you decoupled logging for free.

Two pieces from `std::log`:

- **`Logger`** — a publishing locus with a cascading
  namespace.
- **`StdoutSink`** — the default sink that subscribes to
  `log.**` and prints every event.

## A two-line server with logging

```hale
fn main() {
    std::log::StdoutSink { };          // subscribers register first
    let log = std::log::Logger { name: "app" };

    log.info("starting on :8080");
    log.warn("disk 78% full");
    log.error("connection refused");
}
```

Output:

```
INFO  [app] starting on :8080
WARN  [app] disk 78% full
ERROR [app] connection refused
```

The **ordering rule** is load-bearing: instantiate the sink
**before** any publisher. Bus subscriptions are registered at
the subscriber's `birth()`, and publishes fire immediately on
the publishing call. A sink born after the first publish
misses everything before it.

## Logger levels

`Logger` exposes five methods, one per level:

| Method | Level | Conventional use |
|---|---|---|
| `trace(msg)` | 5 | fine-grained per-step trace |
| `debug(msg)` | 4 | development diagnostics |
| `info(msg)`  | 1 | normal operational events |
| `warn(msg)`  | 2 | recoverable surprises |
| `error(msg)` | 3 | failures that need attention |

Levels are integer constants pending enum-variant patterns.
The numeric ordering above is the level value the sink sees
in the `LogEvent.level` field — it's intentionally
non-monotonic in declaration order (info=1, warn=2, error=3,
then trace/debug at the high end) because `StdoutSink`'s
default filter starts at 1 (info+).

## Cascading namespaces

A `Logger` with no `parent_path` publishes on
`log.<name>`. Set `parent_path` to nest:

```hale
fn main() {
    std::log::StdoutSink { };

    let app_log = std::log::Logger { name: "app" };
    let db_log  = std::log::Logger { name: "db", parent_path: "app" };
    let q_log   = std::log::Logger { name: "queries", parent_path: "app.db" };

    app_log.info("starting");
    db_log.info("connected to postgres");
    q_log.warn("slow query: SELECT * FROM ...");
}
```

The publish subjects are `log.app`, `log.app.db`,
`log.app.db.queries`. A single sink subscribed to `log.**`
catches them all.

## Filtering at the sink

A sink that only wants the `app.db` subtree subscribes
narrowly:

```hale
locus DbAuditSink {
    bus { subscribe "log.app.db.**" as on_event of type std::log::LogEvent; }

    fn on_event(e: std::log::LogEvent) {
        if e.level <= 3 {              // info, warn, error — drop debug/trace
            println("[", e.path, "] ", e.msg);
        }
    }
}
```

Wildcards (`**`) catch every descendant. Exact subjects
(`log.app.db`) catch only that path. This is the same
hierarchical-topic + wildcard mechanism documented in
[The bus](../concepts/the-bus.md) — `std::log` is just an
ordinary user of it.

## Per-locus loggers

A common shape: each long-lived service locus owns a
named `Logger`, instantiated alongside it in `main`.
Loggers are themselves let-bound; the publishing locus
captures the logger name in its params, and a single sink
upstream sees every event.

```hale
locus Routes {
    params { log_name: String = "http"; }

    fn handle(req: std::http::Request) -> std::http::Response {
        let log = std::log::Logger { name: self.log_name };
        log.info(f"{req.method} {req.path}");
        return std::http::Response { status: 200, body: "ok" };
    }
}

fn main() {
    std::log::StdoutSink { };
    std::http::Server { port: 8080, handler: Routes { } };
}
```

The `f"..."` string is f-string interpolation —
`{expr}` evaluates the expression and concatenates the
result into the message.

For long-running services that emit a lot of events,
instantiate the Logger once in `birth()` and store its
`name` field on `self` (the cheapest part of a Logger is the
publish itself — re-instantiating per-call is fine for low
volumes too, since Loguers are statement-position fire-and-
forget).

## Logging from libraries

A library that wants to emit log events doesn't need its
caller to "pass a Logger in." It can instantiate its own
`Logger` with a library-scoped name; consumers subscribe to
the subtree they care about (or just `log.**`) without the
library needing to know.

```hale
// inside a parser library
locus Parser {
    params { log: std::log::Logger = std::log::Logger { name: "parse" }; }

    fn parse(input: String) -> Result {
        self.log.debug(f"parsing {to_string(len(input))} bytes");
        // ...
    }
}
```

## Cross-process logging

Because `Logger.publish` is just a bus publish, you can route
log events between binaries by binding the `log.**`
wildcard topic in `main locus`. See
[Run a topic across binaries](./multi-binary-bus.md) for the
mechanism. The application code doesn't change — same
`log.info(msg)` calls; the deployment seam decides whether
events stay in-process or flow to a remote aggregator.

## What `std::log` doesn't do

- **No log files.** `StdoutSink` is the only built-in sink.
  Write your own subscriber locus + `std::io::fs::write_file_append`
  for file output.
- **No log rotation.** Run `logrotate` against the output.
- **No structured fields.** `LogEvent.msg` is just a String.
  Use f-strings to interpolate values, or emit JSON-shaped
  messages with `std::json::Builder` if downstream consumers
  parse structured logs.

## See also

- [The bus](../concepts/the-bus.md) — hierarchical topics and
  wildcards.
- [Run a topic across binaries](./multi-binary-bus.md) — for
  cross-process log aggregation.
