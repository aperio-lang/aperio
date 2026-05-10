# io-demo

Phase 1 capstone. Combines `std::io::fs` and `std::io::tcp`
into a small program that exercises every primitive the v1.x
stdlib's first arc ships.

```aperio
fn main() {
    let config_path: String = "/tmp/aperio_io_demo_config.txt";
    let log_path: String = "/tmp/aperio_io_demo_log.txt";

    let mut payload: String = "default visit\n";
    if std::io::fs::file_exists(config_path) {
        payload = std::io::fs::read_file(config_path);
        println("config: loaded from ", config_path);
    } else {
        println("config: none, using default");
    }

    println("io-demo: listening on 127.0.0.1:9876");
    std::io::tcp::Listener {
        host: "127.0.0.1",
        port: 9876,
    };

    let r: Int = std::io::fs::write_file(log_path, payload);
    if r == 0 {
        println("io-demo: wrote log to ", log_path);
    } else {
        println("io-demo: log write failed");
    }
}
```

## What runs

1. The program checks for a config file at
   `/tmp/aperio_io_demo_config.txt`. If present, its contents
   become the log payload; otherwise a default string is used.
2. A `std::io::tcp::Listener` binds `127.0.0.1:9876` and
   blocks on accept inside its `run()` lifecycle.
3. When a peer connects, the Listener prints the accepted fd
   and closes the connection (Phase-1 single-accept shape per
   `docs/std/src/io/tcp.md`).
4. The program writes the log payload to
   `/tmp/aperio_io_demo_log.txt` and prints where it landed.
5. `main()` returns. Process exits.

## Running it manually

```
aperio run examples/io-demo/main.ap
```

In a second terminal:

```
nc 127.0.0.1 9876
```

After the connection drops, check the log:

```
cat /tmp/aperio_io_demo_log.txt
```

To exercise the config path, write to the config file before
running:

```
echo "custom payload" > /tmp/aperio_io_demo_config.txt
aperio run examples/io-demo/main.ap
cat /tmp/aperio_io_demo_log.txt   # → custom payload
```

## Primitives this exercises

- **`std::io::fs::file_exists`** — Bool probe.
- **`std::io::fs::read_file`** — String return, allocated in
  the lazy global payload arena.
- **`std::io::fs::write_file`** — Int (0/-1) return.
- **`std::io::tcp::Listener`** — stdlib locus with a real
  three-stage lifecycle (`birth` binds, `run` accepts,
  `dissolve` closes) backed by the m73b `__listen_socket` /
  `__accept_one` / `__close_fd` path-call primitives.
- **Magic `std::*` paths** — every stdlib reference goes
  through the m71 path resolver. No `import`, no `use`.
- **Stdlib-loci-via-bundled-source** — the Listener's
  declaration lives in `runtime/stdlib.ap`, concatenated to
  this program at codegen time per the m73a mechanism.
- **`if` / `else` / `let mut` reassignment** — ordinary
  Aperio surface, exercised against stdlib return values.

## Phase-1 limitations this honestly inherits

- The Listener accepts exactly one connection then exits.
  Servers that handle many connections wait on the
  multi-accept arc (see `docs/std/src/io/tcp.md`'s
  Limitations section).
- The port and paths are hardcoded — Aperio doesn't have
  argv parsing or string-to-int yet, so config-driven values
  beyond raw String contents need plumbing landing later.
- Binary payloads with embedded NULs would truncate at
  `write_file` time. UTF-8 strings only for v0.

## Integration test

`crates/aperio-codegen/tests/io_demo.rs` builds this example,
runs it twice (default-config and seeded-config), and asserts
on stdout + on-disk log contents — same path a user would
exercise manually, automated for CI.
