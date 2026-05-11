# cli-demo

A Cobra-equivalent CLI library for Aperio, designed lotus-shaped.

## Framing

The user's framing:

> ok - think of like, the go cobra cli that we use in
> ~/code/grease. it'd be nice to have a lotus-layer designed
> application entrypoint library defined for aperio as well

And the direction:

> it should be locus all the way down

That's what cli.ap ships: a registry-shaped library locus
(`CliL`) plus a per-subcommand locus pattern for the actual
work. Each subcommand is a real locus with `birth/run/dissolve`,
contracts, bus participation if it needs them — every layer of
the catalog applies, recursively.

## Lotus inventory

- **`CliL`** (`cli.ap`) — the library. Namespace-shape locus
  per `std::cli::Resolver`. Holds command + flag registries as
  parallel-array params (the v0 workaround for missing struct-
  array storage). Builder API to register commands and flags,
  `parse_argv()` to walk `std::env::arg`, query API to read
  resolved flag values + positionals, help printers.
- **`CmdServeL` / `CmdBuildL` / `CmdCheckL`** (`cmds.ap`) — demo
  subcommand loci. Each takes its resolved flag values + any
  positionals as params, fires its `run()` lifecycle. Real apps
  drop in their own subcommand loci here; the library doesn't
  care what the lifecycle bodies do.

## Mapping to Cobra

| Cobra concept             | Aperio shape                                      |
|---------------------------|---------------------------------------------------|
| `cobra.Command{Use,Short,Long,RunE}` | One `CmdXxxL` locus per subcommand     |
| `rootCmd.AddCommand(c)`   | `cli.register_command("name", short)`             |
| `cmd.PersistentFlags()`   | `cli.add_flag(long, short, ty, default, desc)`    |
| `cmd.Flags()` (local)     | `cli.add_cmd_flag(cmd, long, short, ty, default, desc)` |
| `flag.StringVarP(...)`    | `cli.flag_str(name)` after `parse_argv()`         |
| `flag.IntVarP(...)`       | `cli.flag_int(name)`                              |
| `flag.BoolVarP(...)`      | `cli.flag_bool(name)`                             |
| `c.Execute()` dispatch    | `if cli.parsed_cmd == "serve" { CmdServeL{...}; }`|
| Stock help template       | `cli.print_help()` / `cli.print_cmd_help(cmd)`    |

The dispatch is hand-rolled rather than reflective because
Aperio v0 has no way to map a String to a locus type. Each new
subcommand edits three places: `register_command`, `add_cmd_flag`
(once per local flag), and the dispatch arm.

## Three-step pattern

Every CLI built on `CliL` follows the same shape:

1. **Instantiate `CliL`** with name/version/short/long.
2. **Register** — persistent flags first via `add_flag`, then
   each subcommand via `register_command` + its `add_cmd_flag`s.
3. **Parse + dispatch** — `cli.parse_argv()` populates parsed
   state; the if/else chain on `cli.parsed_cmd` instantiates
   the matching `CmdXxxL` with resolved flag values.

See `main.ap` for the worked example.

## How to run

```
cargo build --release -p aperio-cli
target/release/aperio build apps/cli-demo/
```

A representative invocation matrix:

```
apps/cli-demo/cli-demo                              # → root help
apps/cli-demo/cli-demo --version                    # → "cli-demo 0.1.0"
apps/cli-demo/cli-demo --help                       # → root help
apps/cli-demo/cli-demo serve --help                 # → per-command help
apps/cli-demo/cli-demo serve                        # → defaults
apps/cli-demo/cli-demo serve --port 9090            # → port=9090
apps/cli-demo/cli-demo serve -p 9090 -h 0.0.0.0     # → short forms
apps/cli-demo/cli-demo serve --port=9090            # → --long=value form
apps/cli-demo/cli-demo serve --tls --verbose        # → persistent flag works
apps/cli-demo/cli-demo build --target main --release
apps/cli-demo/cli-demo check --strict ./input.ap
apps/cli-demo/cli-demo bogus                        # → unknown cmd, exit 2
```

Each subcommand prints what it received and what it would do.
Real apps would replace the printing with actual work — e.g.,
`CmdServeL.run()` instantiates a `WsServerL` from `apps/ws-echo/`,
`CmdBuildL.run()` calls into a compiler pipeline.

## What's shipped (v0)

- One-level subcommand tree (root → leaf, no nested).
- Persistent + per-subcommand flag scopes.
- Bool / String / Int flag types.
- `--long value`, `--long=value`, `-short value` parsing.
- `--help` (root + per-command), `--version`.
- `--` end-of-flags marker; positional args after subcommand.
- Unknown-command diagnostic + exit 2.
- Stock help formatter; no templates.
- Up to 16 commands × 64 flags total.

## What's not shipped

- **Nested subcommands** (`mytool db migrate up`). The
  parallel-array shape doesn't extend cleanly to a tree.
- **Required-flag enforcement.** Defaults satisfy every flag;
  no `required: bool` field. The locus that consumes the flag
  is the natural place to validate at v0.
- **Custom help templates / examples.** Stock format only.
- **Pre-run / post-run hooks.** Each subcommand locus's
  `birth()` is its own setup hook; a global pre-run goes in
  `CliL.parse_argv()` epilogue if needed.

## v0 constraints captured in code

- **Parallel-array storage everywhere.** `self.cmd_names[i] =
  x` doesn't lower yet (the `self-array-field-index-assign-
  unsupported` friction); every write is a copy-out / mutate /
  write-back of the whole array. Builder methods are 10+ lines
  each.
- **Subcommand registry is metadata-only.** User-defined `type`
  records can't carry fn-pointer fields (logged here in
  `FRICTION.md` as `type-records-cannot-hold-fn-pointer-fields`),
  so the library can't hold a `[Command; N]` array with each
  entry carrying its own `run: fn(...)`. The lotus-all-the-way-
  down architecture sidesteps it by putting the run callback
  IN the subcommand-locus's lifecycle instead.
- **String-typed flag values.** No per-flag generic storage;
  Int flags parse on read via `std::str::parse_int`.

## Cross-references

- `apps/cli-demo/FRICTION.md` — friction entries.
- `notes/aperio-friction.md` — global friction log.
- `crates/aperio-codegen/runtime/stdlib/cli.ap` —
  `std::cli::Resolver`. The new library is a layer up:
  Resolver does flag-value resolution, CliL does command-tree
  registration + parser + help formatting + scope-aware flag
  lookup. Resolver could be promoted to be CliL's inner flag
  resolver in a future round.
- `apps/onboard/main.ap:1250-1260` — Resolver use-site; what
  cli-demo replaces for apps with >1 subcommand.
- `apps/ferryman/main.ap:1675+` — current canonical hand-rolled
  if/else dispatch shape this library cleans up.
- `apps/market-book/book.ap` — parallel-array storage reference
  the library inherits.
- `~/code/grease/cmd/cli/main.go` — Cobra reference grease uses.
