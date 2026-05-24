# Read CLI args & config

Three layers ship in stdlib, ordered low-ceremony to
high-ceremony:

1. **`std::env::*`** — direct argv + environment access.
2. **`std::env::arg_or`** — the one-line "argv with fallback"
   sugar that covers most apps.
3. **`std::cli::Resolver`** — layered config that walks argv,
   env, and defaults in one ritual.

Use the simplest one that fits.

## Direct argv & environment

```hale
fn main() {
    let count = std::env::args_count();    // includes argv[0]
    let mut i = 0;
    while i < count {
        println("argv[", to_string(i), "] = ", std::env::arg(i));
        i = i + 1;
    }

    if std::env::var_exists("HOME") {
        println("HOME = ", std::env::var("HOME"));
    }
}
```

- `std::env::args_count() -> Int` — argv length, including
  the binary name at index 0.
- `std::env::arg(i) -> String` — the i-th argv. Returns `""`
  if out of range.
- `std::env::var(name) -> String` — environment variable, or
  `""` if unset.
- `std::env::var_exists(name) -> Bool` — distinguishes "set
  but empty" from "not set."

## The `arg_or` sugar

For "argv if present, else default" — the most common shape —
use `arg_or`:

```hale
fn main() {
    let port    = std::str::parse_int(std::env::arg_or(1, "8080")) or 8080;
    let host    = std::env::arg_or(2, "127.0.0.1");
    let log_dir = std::env::arg_or(3, "/tmp/logs");

    println("listening on ", host, ":", to_string(port));
}
```

`arg_or(i, default)` returns argv[i] if `args_count() > i`,
otherwise `default`. It collapses the three-line
`if args_count() > 1 { ... }` pattern.

For numeric args, chain a `parse_int(s) or fallback`. The
`or fallback` part is required — `parse_int` returns
`fallible(ParseError)` so the typechecker enforces handling
even if you "know" the input is numeric.

See [Read & write files](./files-and-fallible.md) for the full
treatment of `fallible(E)` and `or` clauses.

## Layered config with `std::cli::Resolver`

When an app reads from argv **and** env **and** falls back to
hardcoded defaults — the standard Unix config layering — use
`std::cli::Resolver`. It holds one source-of-truth for the
precedence rule and exposes a single `get(key, default)`
method.

```hale
type AppConfig {
    host:      String;
    port:      Int;
    log_level: Int;
}

locus App {
    params { cfg: AppConfig; }
    run() {
        println("listening on ", self.cfg.host, ":", to_string(self.cfg.port));
    }
}

fn main() {
    let r = std::cli::Resolver {
        env_prefix: "MYAPP_",
        argv_keys:  "host\nport\nlog_level\n",
    };
    let cfg = AppConfig {
        host:      r.get("host",          "127.0.0.1"),
        port:      r.get_int("port",      8080),
        log_level: r.get_int("log_level", 1),
    };
    App { cfg: cfg };
}
```

Precedence (highest wins):

1. **CLI positional argv.** `argv_keys` is a newline-separated
   list naming each positional slot; `argv[1]` is the first
   key, `argv[2]` is the second, etc.
2. **Environment.** `<env_prefix><KEY_UPPERCASE>` — so `host`
   reads `MYAPP_HOST`, `log_level` reads `MYAPP_LOG_LEVEL`.
3. **Default.** The second arg to `get` / `get_int`.

Empty strings at a higher layer fall through to the next layer
(an empty env var is not "explicitly empty" — it's "not set").

## When to reach for which

| You're writing | Use |
|---|---|
| A demo, a one-off CLI tool | `std::env::arg_or` |
| A daemon read from env in production | `std::cli::Resolver` |
| A tool with positional args + flags | `std::cli::Resolver` |
| Anything that needs `--flag value` style parsing | Roll your own loop over `std::env::arg(i)` — no flag parser in stdlib at v1. |

The "no flag parser" gap is deliberate: Resolver covers the
common "positional + env + default" shape; tools that need
`--verbose --log-level=info` parsing are rare enough that the
right place is a contrib library, not stdlib.

## See also

- [Structured logging](./logging.md) — `log_level` from
  Resolver flows naturally into `Logger` construction.
- [Project layout](./project-layout.md) — running the binary
  vs the interpreter.
