<!-- Synced from aperio-lang/pond/agent/sandbox/README.md by tools/sync-pond-docs.sh — do not edit here. -->

# pond/agent/sandbox — subprocess-based code execution sandbox

Suggested import alias: **`sandbox`**

```aperio
import "vendor/pond/agent/sandbox" as sandbox;
```

## Status (2026-05-16): BLOCKED

This library cannot ship a real implementation at v1. Its sole
runtime dependency `pond/subprocess` is itself **BLOCKED** on a
missing stdlib spawn primitive — the Aperio stdlib's
`std::process::*` surface only ships `pid()` and `exit(code)`,
and `AGENTS.md` forbids editing `crates/` to add a fork/exec
primitive from a lib-build session. See
`../../subprocess/FRICTION.md` for the proposed `std::process::run`
spec and the C-runtime / codegen additions it needs.

Until both lower libs unblock, every fallible surface in this
lib resolves to:

```
SandboxError { kind: "spawn_failed",
               detail: "pond/subprocess: stdlib spawn primitive not yet available; see FRICTION.md" }
```

The contract from `pond/CONTRACTS.md § pond/agent/sandbox/` is
fully realized at the type / interface / signature level, so
consumer code that compiles against this lib today will compile
unchanged against the real implementation tomorrow. The day
`pond/subprocess::spawn(...)` ships real behavior, the bodies
in `sandbox.ap` flip from forwarding the stub error to
forwarding the actual captured stdout/stderr — without touching
any signature.

## Surface

A single `Sandbox` Service locus (pattern 3) carries the runtime
binary + resource limits across a series of runs; two methods
execute code either inline or from a pre-existing file. Two
free-fn companions wrap the methods for callers that prefer the
value channel.

```aperio
// Construct once, reuse across runs.
let sb = sandbox::Sandbox {
    runtime:         "python3",
    timeout_ms:      5000,
    memory_limit_mb: 256,
};

// Run an inline snippet. (Today: returns the BLOCKED sentinel.)
let r = sb.run_code("print('hello')");
if sb.last_error.kind != "" {
    println("sandbox failed: ", sb.last_error.kind, " — ", sb.last_error.detail);
} else {
    println("exit ", r.exit_code, ": ", r.stdout);
}

// Run a script from disk.
let r2 = sb.run_file("/srv/agent/scripts/probe.py");

// Or use the fallible free-fn wrappers (value channel).
let r3 = sandbox::run_code_at(sb, "print(2 + 2)") or self.report(err);
```

## Types

| Name           | Shape                                                |
|----------------|------------------------------------------------------|
| `SandboxResult`| `exit_code: Int; stdout: String; stderr: String;`    |
| `SandboxError` | `kind: String; detail: String;` (synthesized payload)|
| `Sandbox`      | locus — Service holding runtime + limits             |

## Error shape

`SandboxError.kind` is one of:

- `"timeout"`      — wall-clock `timeout_ms` elapsed; the runtime
                     was killed before completing.
- `"oom"`          — RESERVED for future use; surfaced once
                     `pond/subprocess` plumbs rlimit-style memory
                     enforcement.
- `"spawn_failed"` — the underlying subprocess couldn't start
                     (covers the current "lib is blocked" state,
                     where `detail` forwards the SpawnError's
                     "unsupported" detail string verbatim).
- `"io"`           — failed to write the temp file for `run_code`,
                     or failed to read back the captured output.

`detail` is human-readable; typically forwarded from the
underlying `sub::SpawnError.detail`.

## Contract deviations

- `Sandbox.run_code` / `Sandbox.run_file` are declared *without*
  `fallible(SandboxError)`. Per `spec/semantics.md § Fallible
  call semantics` (and KNOWN_GOTCHAS.md G4) user-declared locus
  methods may not declare `fallible(E)` — that channel is
  reserved for free fns and `@form(...)`-synthesized methods.
  Failures surface via:
  1. `self.last_error: SandboxError` — populated by every call.
     `kind == ""` means success.
  2. The free-fn helpers `run_code_at(sb, code)` /
     `run_file_at(sb, path)` for callers that want the value
     channel (`or raise`, etc.).
  3. The `closure fatal_sandbox { captures: last_error; epoch
     inline; } / violate fatal_sandbox` pair for structural
     drain via `on_failure`.

  See `FRICTION.md` for the wider trend across `pond/CONTRACTS.md`
  — `pond/subprocess` flagged the same pattern as
  duplicate-suspected.

- `memory_limit_mb` is plumbed on the surface but currently a
  no-op. `pond/subprocess` has no rlimit-style hook in its
  current contract; enforcement is deferred to the day the
  stdlib spawn primitive grows a `mem_limit_bytes` field.

## Building

```
$ aperio build \
    pond/agent/sandbox/
```

Type-checks cleanly. Runtime calls are stubs until both
`pond/subprocess` and the underlying stdlib spawn primitive
ship.

## Example

`examples/run-python/main.ap` constructs a Sandbox targeting
`python3`, runs `print('hello')`, and prints the result (which
today is the BLOCKED sentinel; tomorrow is `hello`):

```
$ aperio build \
    pond/agent/sandbox/examples/run-python/
$ ./examples/run-python/run-python
```
