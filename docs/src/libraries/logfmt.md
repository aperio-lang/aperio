<!-- Synced from aperio-lang/pond/logfmt/README.md by tools/sync-pond-docs.sh — do not edit here. -->

# pond/logfmt — drop-in log sinks for `std::log`

Alternative `std::log` sinks that wear the `std::text::Sink` shape
(F.20-structural — no `impl I for L`) and subscribe to `log.**` the
same way `std::log::StdoutSink` does. Use these in place of the
stdlib sink when you want events to land somewhere other than
stdout/stderr.

## Suggested alias

```aperio
import "vendor/pond/logfmt" as logfmt;
```

The bare alias `logfmt` matches `pond/CONTRACTS.md` and
`pond/README.md` § Tier 2.

## Sinks

| Locus | Destination | Status |
|------|-----------|--------|
| `FileSink`  | Appends to a path; size-based rotation | shipped |
| `OtlpSink`  | Batches + posts to an OTLP/HTTP endpoint | **STUB** at v1 (see FRICTION.md) |

Both loci satisfy `std::text::Sink` structurally (`write(s) -> ()`,
`line(s) -> ()`, `newline() -> ()`) AND carry the
`subscribe "log.**"` bus declaration that turns them into log-event
listeners. The dual surface means one locus drop-in-replaces the
StdoutSink at the log-routing layer AND the StdoutSink at the
text-rendering layer.

## Drop-in replacement for `std::log::StdoutSink`

Before:

```aperio
fn main() {
    std::log::StdoutSink { };
    let log = std::log::Logger { name: "app" };
    log.info("hello");          // → "[INFO app] hello" on stdout
}
```

After (file destination):

```aperio
import "vendor/pond/logfmt" as logfmt;

fn main() {
    logfmt::FileSink {
        path: "/var/log/myapp.log",
        max_size_bytes: 10000000,
        keep_files: 5
    };
    let log = std::log::Logger { name: "app" };
    log.info("hello");          // → appended to /var/log/myapp.log
}
```

Either sink may be paired with `std::log::StdoutSink` — both
subscribe to `log.**` so events fan out to every live subscriber.
Subscribers must be instantiated **before** any `Logger` publishes
(per AGENTS.md's bus-ordering rule); put the sink construction at
the top of `main` or the top of the app locus's `run()`.

## `FileSink` — file with rotation

```aperio
locus FileSink {
    params { path: String;
             max_size_bytes: Int = 10000000;
             keep_files:     Int = 5; }

    // std::text::Sink-shape methods
    fn write(s: String);
    fn line(s: String);
    fn newline();

    // log.** subscriber
    fn on_event(e: std::log::LogEvent);

    // last-error accessors (see "two-channel deviation" below)
    fn last_error_kind() -> String;
    fn last_error_errno() -> Int;
    fn last_error_path() -> String;
}
```

Each log event renders to `[LEVEL path] msg\n` (same format
`std::log::StdoutSink` uses) and is appended via
`std::io::fs::write_file_append`. After every append, the locus
calls `std::io::fs::file_size`; if the active path is now larger
than `max_size_bytes`, the chain shifts:

```
path.{keep_files-1} → path.{keep_files}   (oldest is overwritten)
...
path.1              → path.2
path                → path.1
path                ← truncated to ""
```

Each shift is implemented as a read-then-write (see FRICTION.md
`no-rename-no-unlink-in-fs-stdlib`); the bound on every read is
`max_size_bytes`, so rotation cost is linear in the cap, not in
total log volume.

## `OtlpSink` — OTLP/HTTP batch shipper (STUB)

```aperio
locus OtlpSink {
    params { endpoint:     String;
             service_name: String = "aperio-app";
             batch_size:   Int    = 32; }

    // std::text::Sink-shape methods (every call enqueues at INFO)
    fn write(s: String);
    fn line(s: String);
    fn newline();

    // log.** subscriber — preserves the source severity
    fn on_event(e: std::log::LogEvent);

    fn flush();                          // explicit drain
    fn pending_payload() -> String;      // OTLP/JSON the would-be POST
    fn batches_count()    -> Int;
    fn pending_event_count() -> Int;
    fn last_error_kind() -> String;
    fn last_error_status() -> Int;
    fn last_error_detail() -> String;
}
```

The batching, severity mapping (`std::log::LogEvent.level` → OTLP
`severityNumber`), and OTLP/JSON payload assembly via
`std::json::Builder` are wired. **The HTTP POST is stubbed.** See
FRICTION.md `otlp-transport-stubbed` for the gap; the file ends with
the exact swap-in needed to lift the stub once consumers vendor
`pond/http/client` alongside this lib (the v1 no-transitive-import
rule blocks the direct dep).

`pending_payload()` returns the OTLP/JSON that *would* be POSTed,
so a consumer with its own HTTP surface can ship the batch
externally in the meantime.

## Two-channel deviation from CONTRACTS.md

`pond/CONTRACTS.md` lists every Sink method as
`fallible(IoError)`. Per AGENTS.md's two-channel rule, locus methods
on user-declared loci **cannot** declare `fallible(E)` — the value
channel for IO errors has to be wrapped via `or self.method(err)`
inside the method body. Each sink captures the last failure into a
`last_kind` / `last_errno` (or `last_status`) / `last_path` (or
`last_detail`) triple readable through accessor methods, matching
the shape `pond/http/client::Client` uses. See FRICTION.md
`fallible-on-locus-method`.

## Files

- `file_sink.ap` — `FileSink` locus + rotation.
- `otlp_sink.ap` — `OtlpSink` locus (stub transport, real payload).
- `examples/rotated-file/main.ap` — App-locus demo: log 100 events
  with `max_size_bytes: 512`, then walk the rotated chain and
  verify `.1` exists.

## Verification

```bash
# library typechecks (codegen complains about no `fn main()`,
# which is expected for a lib seed)
aperio build \
    pond/logfmt/

# rotation demo — build then run
aperio build \
    pond/logfmt/examples/rotated-file/
pond/logfmt/examples/rotated-file/rotated-file
```

Expected demo output:

```
--- rotated chain state ---
  active log (/tmp/logfmt-rotated.log): exists, size=0
  rotation .1 (/tmp/logfmt-rotated.log.1): exists, size=530
  rotation .2 (/tmp/logfmt-rotated.log.2): exists, size=530
  rotation .3 (/tmp/logfmt-rotated.log.3): exists, size=530
rotated-file: rotation verified
```

(The active log is empty at end-of-run because the final rotation
truncated it and no further events were emitted — exactly the
post-rotation steady state.)
