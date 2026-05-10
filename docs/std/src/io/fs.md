# `std::io::fs`

Filesystem operations for Aperio programs. Phase 1 (m75) ships
four one-shot synchronous functions: `read_file`, `write_file`,
`file_size`, `file_exists`. Each is a path-call function — no
locus wrapping, no streaming handle, no buffering layer. The
shape mirrors `std::process::pid` rather than the Listener-style
locus pattern because file ops are inherently one-shot: there's
no lifetime-of-a-stream concept to manage.

A future milestone that needs streaming reads (large files,
line-by-line processing, tailing logs) adds a separate
streaming family alongside these without disturbing them.

## Functions

### `std::io::fs::read_file`

#### Synopsis

```aperio
fn read_file(path: String) -> String
```

Reads the entire file at `path` and returns its contents as a
String. If the path is missing or unreadable, returns the empty
String. To distinguish "missing" from "empty," probe with
`file_exists` first.

#### Semantics

- Two-phase: stats the file to learn its size, allocates a
  (size + 1)-byte buffer in the lazy global payload arena
  (so the result outlives the call frame), reads into it,
  NUL-terminates at the actual bytes-read offset.
- Returns the empty String on any error (missing path,
  permissions, IO failure). The C substrate's -1 return is
  clamped to 0 in the Aperio surface so callers don't have
  to handle the negative case.
- The returned String references arena memory that lives for
  the program's duration. Repeated reads accumulate; on a
  long-running process this is unbounded growth (acceptable
  for v1 since subscribers run for bounded duration —
  mirrors the m70 deserialize convention).

#### Examples

```aperio
fn main() {
    if std::io::fs::file_exists("config.toml") {
        let contents = std::io::fs::read_file("config.toml");
        println("loaded: ", contents);
    } else {
        println("no config; using defaults");
    }
}
```

### `std::io::fs::write_file`

#### Synopsis

```aperio
fn write_file(path: String, content: String) -> Int
```

Writes `content` to `path`, truncating any existing file.
Returns 0 on success, -1 on error.

#### Semantics

- Opens the path with `O_WRONLY | O_CREAT | O_TRUNC`, mode
  `0644`. Existing files are replaced wholesale; existing
  permissions are preserved (POSIX `open` doesn't change mode
  on existing files).
- Length is computed from the content's String pointer via
  `strlen`. Aperio Strings are NUL-terminated in memory, so
  embedded NULs in payloads silently truncate the write at
  the first NUL. (This mirrors the m70 String wire-format
  contract.)
- Checks `close()`'s return so deferred filesystem errors —
  NFS write-back, ENOSPC surfacing on flush — produce a -1
  rather than being silently dropped.

#### Examples

```aperio
fn main() {
    let log = "request from 127.0.0.1\n";
    let r = std::io::fs::write_file("audit.log", log);
    if r == 0 {
        println("logged");
    } else {
        println("log write failed");
    }
}
```

### `std::io::fs::file_size`

#### Synopsis

```aperio
fn file_size(path: String) -> Int
```

Returns the size of `path` in bytes, or -1 on error. Follows
symlinks (uses `stat`, not `lstat`).

#### Semantics

- Stats the path; returns `st.st_size` cast to Int.
- Errors (missing file, permission denied) collapse to -1.
  Callers that need to distinguish use `file_exists` plus
  errno (the latter not currently surfaced in the Aperio
  layer; a future error-introspection milestone fills that in).

#### Examples

```aperio
fn main() {
    let n = std::io::fs::file_size("CHANGELOG.md");
    if n > 0 {
        println("CHANGELOG is ", n, " bytes");
    }
}
```

### `std::io::fs::file_exists`

#### Synopsis

```aperio
fn file_exists(path: String) -> Bool
```

Returns `true` if `path` exists, `false` otherwise. Follows
symlinks; non-existent symlink targets report `false`.

#### Semantics

- Probes via `stat`. Any error (ENOENT, EACCES, etc.) returns
  `false`. The function does not distinguish between
  "definitively absent" and "couldn't tell."

#### Examples

```aperio
fn main() {
    if std::io::fs::file_exists("/etc/hostname") {
        let h = std::io::fs::read_file("/etc/hostname");
        println("hostname: ", h);
    }
}
```

## Limitations (Phase 1)

- **No streaming**: the entire file is read into memory in
  one call. Large files (hundreds of MB+) are uncomfortable.
- **No `read_dir`**: directory listing is deferred to a
  follow-up milestone — the variable-length-output story
  (NUL-separated buffer? iteration model?) deserves its own
  design pass.
- **NUL-truncation on write**: Aperio Strings are
  NUL-terminated in memory, so writing binary data with
  embedded NULs truncates at the first NUL. Real binary I/O
  waits on a `Bytes` type with proper codegen support.
- **No errno surface**: errors collapse to -1 / `false` /
  empty. A future milestone surfaces errno-style detail.
- **Lazy global arena growth**: every `read_file` call
  allocates from a process-lifetime arena. Long-running
  processes that re-read files repeatedly grow memory
  unbounded.

## See Also

- [Roadmap](../roadmap.md) — Phase 1+ stdlib build-out plan.
- [`std::io::tcp`](./tcp.md) — sibling I/O module for
  network sockets.
- `crates/aperio-codegen/runtime/lotus_arena.c` (in the
  language repo) — POSIX wrappers backing this module.
