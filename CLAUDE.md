# Claude Code entry point

The canonical agent prompt for this repo is [`AGENTS.md`](./AGENTS.md).
Read it first.

`AGENTS.md` targets agents writing `.hl` programs. If you're
working on the language itself (compiler / runtime / spec), the
older role-organized briefs under [`agents/`](./agents/) still
apply:

- [`agents/library-dev.md`](./agents/library-dev.md) — adding to
  the stdlib or writing an Hale library.
- [`agents/compiler-dev.md`](./agents/compiler-dev.md) — working
  on the compiler / runtime / spec.

## Build + test (compiler work only)

```sh
cargo build --release
cargo test --release --workspace -- --test-threads=1
```

The serial flag avoids "text file busy" flakes from parallel
test binaries racing each other on the same temp path.

## Repo conventions

- **Hale** is the language. **lotus** is the runtime substrate.
  C-runtime symbols stay `lotus_*` by design.
- The spec under `spec/` is the canonical contract. It describes
  shipped behavior, not aspirations. If the impl changes
  user-visible behavior, the spec changes in the same commit.
- The user owns commit cadence — never commit without an
  explicit ask.
- Don't generate planning / status / progress markdown files in
  the repo.
