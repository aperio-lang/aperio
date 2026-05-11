# cli-demo friction log

> Per-app friction encountered while building `apps/cli-demo/`.
> The bigger entries already live in the global log at
> `notes/aperio-friction.md`; this file points at them rather
> than duplicating.

## Already-logged friction this app exercises

Two open entries in the global log gate the cleaner version of
this library. They drove the parallel-array storage shape; once
they close, the library is a candidate for promotion to
`std::cli::Cli` + `std::cli::Command` with the natural
Cobra-shaped surface.

### `self-array-field-index-assign-unsupported` (global log)

Every builder method (`register_command`, `add_flag`,
`add_cmd_flag`) and every parse-time mutator does the
copy-out / mutate-locally / write-back-whole-array dance:

```aperio
let mut names = self.cmd_names;
names[i] = name;
self.cmd_names = names;
```

When the compound `self.<arr_field>[i] = x` assignment lowers,
these bodies collapse to one line each. Same pattern as
`apps/market-book/book.ap`'s `_set_bid` / `_set_ask` mutators.

### user-types-with-fn-pointer-fields (newly observed; logged
below)

The natural Cobra shape — `type Command { run: fn(CliCtxL); }`
— doesn't compile. Aperio v0 allows fn-pointer fields on
*locus* params (canonical: `std::io::tcp::Listener.on_connection`)
but not on `type` records. That forced the library into the
"each subcommand is its own locus + app does dispatch by name"
shape — which the user explicitly wanted ("lotus all the way
down"), so it ended up an alignment with the substrate
constraint rather than a workaround. Both designs would be
viable once user-types can hold fn-pointers; the library would
then offer the choice.

## New entries (this app)

## 2026-05-11 type-records-cannot-hold-fn-pointer-fields

**Tried:** Declare `type Command { name: String; short:
String; run: fn(CliCtxL); }` so that an app could write
`let serve_cmd = Command { name: "serve", run: cmd_serve };`
and register an array of these into a CliL. This is the
canonical Cobra shape (one struct per command).
**Hit:** The typechecker rejects fn-pointer fields on `type`
records. Searched across every user-type declaration in
`apps/*/*.ap` and `crates/aperio-codegen/runtime/stdlib/*.ap`:
zero fn-pointer fields on types. Only loci carry them
(`Listener.on_connection`, `Walk.on_file`, free-fn callback
params like `__handle_one_connection`).
**Workaround:** Each subcommand is its own locus. CliL holds
parallel-array storage of command + flag *metadata only* (no
run callback). The app's main() does `if cli.parsed_cmd ==
"serve" { CmdServeL { ...resolved flags... }; }` — the locus
literal at statement position fires the lifecycle. Functional
and lotus-shaped; the trade is that a new subcommand needs
edits in three places (register_command, add_cmd_flag×N,
dispatch arm).
**Why it matters:** Compounds with
`self-array-field-index-assign-unsupported` to push every
metadata-bearing-struct shape (registries, plugin tables,
callback dispatch tables, even something as simple as a
`Route { path, handler }` table for HTTP routing) toward the
parallel-array workaround. The Cobra-equivalent library has
the most natural one-struct-per-command shape blocked. The
single-primitive minimum to unblock: lift the
"fn-pointer-field" restriction on `type` records — the same
fn-pointer carrier locus params already use should work for
type fields. Codegen-side this is presumably a one-line
relaxation in the type-checker plus matching the existing
fn-pointer field-storage shape from loci.

## 2026-05-11 dense-locus-storage-bloat

**Tried:** Carry the registry inside CliL's params: 16
commands × 6 metadata Strings + 64 flags × 6 metadata Strings.
The natural defaults `[""; 16]` and `[""; 64]` produce a
locus that's ~480 String slots wide.
**Hit:** Compiles fine — Phase 2d's `[val; N]` array-repeat
makes the defaults compact in source — but every CliL
instance allocates the full ~480 slots even when an app
registers only 3 commands and 8 flags. The serialization /
deep-copy cost is correspondingly proportional to the cap,
not the actual usage. Not a build error; a fit-for-purpose
mismatch.
**Workaround:** Live with it at v0. The bloat is per-CliL-
instance and there's typically one per process, so absolute
memory is ~tens of KB. Caps can shrink in apps that need
smaller (16 / 64 are generous defaults; cli-demo uses 3 / 8).
**Why it matters:** The proper shape is growable storage —
either a `List<String>` generic (gates on generics) or a
`bytes::concat`-style appendable buffer with offset indexing
(gates on `bytes-construction-from-ints` from ws-echo). v0
ceiling. Not blocking; flagged so the next iteration knows
which pieces collapse together.
