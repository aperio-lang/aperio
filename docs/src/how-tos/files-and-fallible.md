# Read & write files

`std::io::fs::*` is the filesystem surface. Every operation
except `file_exists` returns `fallible(IoError)` — the call
site has to address the failure with an `or` clause before the
typechecker will let you consume the value. This page covers
the surface and the four addressing motions.

## The surface

| Call | Returns | Notes |
|---|---|---|
| `read_file(p)` | `String fallible(IoError)` | UTF-8 text |
| `read_bytes(p)` | `Bytes fallible(IoError)` | binary |
| `write_file(p, s)` | `() fallible(IoError)` | overwrites |
| `write_file_append(p, s)` | `() fallible(IoError)` | appends |
| `file_size(p)` | `Int fallible(IoError)` | bytes |
| `mkdir(p)` | `() fallible(IoError)` | parents must exist |
| `file_exists(p)` | `Bool` | **NOT fallible** — predicate |
| `list_dir_count(p)` | `Int fallible(IoError)` | entry count |
| `list_dir_at(p, i)` | `String fallible(IoError)` | i-th entry name |

`IoError` carries:

- `kind: String` — `"not_found"`, `"permission_denied"`,
  `"is_dir"`, `"already_exists"`, `"broken_pipe"`, etc.
  (errno-derived; `"io"` is the catch-all.)
- `errno: Int` — raw platform errno.
- `path: String` — the file path the call was made against.

## The five addressing motions

Pick the disposition that matches your intent — the
typechecker rejects an unaddressed fallible call.

### `or raise` — propagate

The enclosing function must itself be `fallible(IoError)` (or
a compatible payload), so the error has somewhere to go.

```aperio
fn load_config() -> String fallible(IoError) {
    return std::io::fs::read_file("config.toml") or raise;
}
```

### `or <value>` — substitute

Provide a fallback value of the success type. `err` is in
scope inside the fallback expression.

```aperio
let body = std::io::fs::read_file("welcome.txt") or "(no welcome message)";
let size = std::io::fs::file_size(path)         or 0;
```

The fallback's type must match the success type. Substituting
`""` for `read_file` works because `read_file` returns
`String`; substituting `0` for `mkdir` is a type error
(`mkdir` returns `()`) — use `or discard` instead.

### `or self.handler(err)` — hand off

Call a member function on the current locus that takes the
error and returns the success type. Useful when several call
sites share a recovery policy.

```aperio
locus Importer {
    params { failed: Int = 0; }

    fn handle_io(e: IoError) -> String {
        self.failed = self.failed + 1;
        eprintln("skipped ", e.path, ": ", e.kind);
        return "";
    }

    fn process(p: String) {
        let body = std::io::fs::read_file(p) or self.handle_io(err);
        if len(body) > 0 { /* ... */ }
    }
}
```

The member fn IS a real function — pick a descriptive name
(`handle_io`, `recover_index`), not a placeholder. See
[The two failure channels](../concepts/failure.md) §
"Bridging the channels" for the pattern that lets the handler
escalate via `violate NAME` instead of substituting.

### `or discard` — swallow (Unit-only)

For calls whose success type is `()`, when you genuinely
don't care:

```aperio
std::io::fs::mkdir("/tmp/cache") or discard;     // ok if it already exists
```

`or discard` is rejected on value-bearing calls — the
typechecker tells you "this returns `String`, can't discard"
and suggests `or ""` or `or raise`.

### `or fail <payload>` — translate to your error type

Symmetric to `or raise`, but you supply a fresh payload of the
enclosing fallible fn's declared error type instead of
forwarding the inner call's `IoError` verbatim. Useful when
your library has its own error vocabulary and you don't want
to leak `IoError` through it.

```aperio
type ConfigErr { reason: String; path: String; }

fn load_config(p: String) -> Config fallible(ConfigErr) {
    let body = std::io::fs::read_file(p)
        or fail ConfigErr { reason: "read failed", path: p };
    return parse(body) or fail ConfigErr { reason: "parse", path: p };
}
```

The enclosing fn must itself be `fallible(T)`; outside one,
the typechecker rejects with a hint to use `or raise` or
`or <fallback>`. Diverges like `or raise` — the chain value
collapses to the inner call's success type.

## A worked example: copy + count

A small CLI that reads every `.md` file in a directory, counts
total bytes, and writes the count to `out.txt`. Every fallible
call is addressed; one helper propagates with `or raise`.

```aperio
fn count_markdown(dir: String) -> Int fallible(IoError) {
    let count = std::io::fs::list_dir_count(dir) or raise;
    let mut total = 0;
    let mut i = 0;
    while i < count {
        let name = std::io::fs::list_dir_at(dir, i) or raise;
        if std::str::index_of(name, ".md") > 0 {
            let path = dir + "/" + name;
            let sz   = std::io::fs::file_size(path) or 0;
            total = total + sz;
        }
        i = i + 1;
    }
    return total;
}

locus App {
    params { dir: String = "."; }

    fn handle_io(e: IoError) -> Int {
        eprintln("count failed at ", e.path, ": ", e.kind);
        return -1;
    }

    run() {
        let total = count_markdown(self.dir) or self.handle_io(err);
        if total < 0 { return; }
        std::io::fs::write_file("out.txt", f"total bytes: {to_string(total)}\n")
            or discard;
        println("counted ", to_string(total), " bytes in ", self.dir);
    }
}

fn main() {
    let dir = std::env::arg_or(1, ".");
    App { dir: dir };
}
```

## Why every call is fallible

Every filesystem call can fail for reasons outside the
caller's control: a directory disappears between
`list_dir_count` and `list_dir_at`; permissions change; the
disk fills. The two-channel rule (see
[The two failure channels](../concepts/failure.md)) puts
these on the value channel because the caller — not the
parent locus — is the right place to decide what to do
(retry, skip, escalate).

If you find yourself writing `or raise` on every line, your
function probably wants to be the propagation boundary —
declare it `-> T fallible(IoError)` and let the wrapper choose
the policy.

## See also

- [The two failure channels](../concepts/failure.md) — the
  conceptual framing and the structural channel.
- [Read & write JSON](./json.md) — for parsing the strings
  `read_file` returns.
- [Standard library](../reference/stdlib.md#path-call-dispatch) —
  the canonical surface listing.
