<!-- Synced from aperio-lang/pond/subprocess/README.md by tools/sync-pond-docs.sh — do not edit here. -->

# pond/subprocess — fork/exec with pipes + timeout

Suggested import alias: **`sub`**

```aperio
import "vendor/pond/subprocess" as sub;
```

## Status (2026-05-16): BLOCKED

This library cannot ship a real implementation at v1. The Aperio
stdlib does not currently expose a subprocess spawn primitive
(`std::process::*` covers `pid()` and `exit(code)` only), and
`AGENTS.md` forbids editing `crates/` to add one. Every fallible
surface in this lib currently fails uniformly with:

```
SpawnError { kind: "unsupported", detail: "pond/subprocess: stdlib spawn primitive not yet available; see FRICTION.md", errno: 0 }
```

The contract from `pond/CONTRACTS.md § pond/subprocess/` is
fully realized at the type / interface / signature level, so
consumer code that compiles against this lib today will compile
unchanged against the real implementation tomorrow. The day
`std::process::spawn(...)` (or equivalent — see `FRICTION.md`
for the proposed spec) ships in `runtime/stdlib/`, the bodies
flip from `fail SpawnError { kind: "unsupported", ... }` to the
real fork/exec/pipe drive code without touching any signature.

## Surface

Two free-fn entry points for the one-shot case, plus a `Process`
locus + three topics for the long-lived streaming case.

```aperio
// One-shot — run to completion, capture full output.
// (Contract deviation: CONTRACTS.md names this `run`, but
// `run` is a reserved lifecycle keyword. See FRICTION.md § 3.3.)
let out = sub::run_cmd("/bin/ls", "-la /tmp") or raise;
println(len(out.stdout), " bytes on stdout");

// Same with full opts.
let opts = sub::SpawnOpts {
    cmd:        "/usr/bin/grep",
    args:       "-i needle",
    cwd:        "/srv/data",
    env:        "PATH=/usr/bin\tLANG=C.UTF-8",
    stdin:      haystack_bytes,
    timeout_ms: 5000,
};
let out = sub::spawn(opts) or self.report_err(err);

// Long-lived — Process locus + bus topics.
locus Watcher {
    bus { subscribe sub::StdoutLine as on_line; }
    fn on_line(s: String) { println("child said: ", s); }
}
let p = sub::Process { cmd: "/usr/bin/tail", args: "-f /var/log/app.log" };
p.send_stdin(std::bytes::from_string(""));
let st = p.wait();
```

## Types

| Name | Shape |
|---|---|
| `SpawnOpts`   | `cmd, args, cwd, env, stdin, timeout_ms` |
| `ExitStatus`  | `code, signaled, signal` |
| `Output`      | `status, stdout, stderr` |
| `SpawnError`  | `kind, detail, errno` (synthesized fallible payload) |
| `Process`     | locus — long-lived streaming child |
| `StdoutLine`  | topic, `payload: String` |
| `StderrLine`  | topic, `payload: String` |
| `ProcessExit` | topic, `payload: ExitStatus` |

## Error shape

`SpawnError.kind` is one of:

- `"unsupported"` — stdlib spawn primitive not yet available
  (every call today).
- `"spawn_failed"` — fork/exec didn't take.
- `"io"` — pipe read/write failure on stdin/stdout/stderr.
- `"timeout"` — `timeout_ms` elapsed before child reaped.
- `"killed"` — child was signaled, no exit code recovered.

`errno` carries the raw platform errno; `detail` is human-readable.

## Contract deviations

- `Process.send_stdin` / `signal` / `wait` are declared without
  `fallible(SpawnError)`. Per `spec/semantics.md § Fallible call
  semantics`, locus methods on user-declared loci may not declare
  `fallible(E)` — that channel is reserved for free fns and
  `@form(...)`-synthesized methods. Errors surface via
  `self.last_error: SpawnError` and via the inline-`violate`
  pattern from `spec/styleguide.md § 7`. See `FRICTION.md`
  for the wider trend across `pond/CONTRACTS.md`.

## Building

```
$ aperio build \
    pond/subprocess/
```

Type-checks cleanly. Runtime calls are stubs until the stdlib
gap closes.
