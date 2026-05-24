# Contributing

Hale's language surface is stable. A few small additions are
planned, but most work between now and v1 is bugs, stability,
and performance.

## Picking a role

The contributor flow is organized by role. Pick the one that
matches what you're trying to do, and read the corresponding
brief at the repo root:

- [`AGENTS.md`](https://github.com/hale-lang/hale/blob/main/AGENTS.md) —
  if you're writing an Hale program (also the load-bearing
  prompt for AI agents authoring `.hl` code).
- [`agents/library-dev.md`](https://github.com/hale-lang/hale/blob/main/agents/library-dev.md) —
  if you're extending the stdlib or writing a reusable Hale
  library.
- [`agents/compiler-dev.md`](https://github.com/hale-lang/hale/blob/main/agents/compiler-dev.md) —
  if you're working on the compiler or runtime itself.

Each brief is self-contained. Read the one for your task; you
shouldn't need the others.

## Running the test suite

Before opening a PR:

```sh
cargo build --release
cargo test --release --workspace -- --test-threads=1
```

The `--test-threads=1` flag is load-bearing — parallel test
binaries can race each other on the same temp paths and surface
flaky "text file busy" failures. Run tests serially.

The test suite is the source of truth for what the compiler
supports. If you're changing a language feature, add a test
under `crates/hale-codegen/tests/` that exercises the new
behavior. If you're changing the parser, the
`crates/hale-syntax/tests/examples.rs` test will exercise
your change against every example fixture.

## Spec discipline

Surface-language and runtime behavior is documented in `spec/`.
If you change behavior, update the spec in the same commit.
The spec is **not aspirational** — it describes what's shipped.
