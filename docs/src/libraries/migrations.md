<!-- Synced from aperio-lang/pond/migrations/README.md by tools/sync-pond-docs.sh — do not edit here. -->

# pond/migrations — schema migration runner

Suggested import alias: **`migs`**

```aperio
import "vendor/pond/migrations" as migs;
import "vendor/pond/sqlite"     as db;
```

## Status (2026-05-16): consumer-ready, runtime-blocked

The library itself builds and runs (`aperio build pond/migrations/`
produces a working `migrate` binary). Iteration, sorting,
pending-detection, and the up/down driver all work end-to-end —
verified against the bundled `examples/blog-schema/` demo.

Every actual SQL operation routes through `pond/sqlite::exec` /
`query_one`, which is **architecturally blocked** on a stdlib
sqlite primitive (see `pond/sqlite/FRICTION.md`). So today every
`migrate up` call applies zero migrations and surfaces a
diagnostic line; the day sqlite unblocks, no source change in
`pond/migrations/` is needed for migrations to actually execute.

## Migration file naming convention

```
<dir>/
    001_create_posts.sql        — apply this migration
    001_create_posts.down.sql   — roll this migration back
    002_add_comments.sql
    002_add_comments.down.sql
    ...
```

- **Prefix is the version**: zero-padded integer (`NNN_`). Padding
  to 3 digits is the convention; ASCII sort on the padded prefix
  matches numeric order, which is how the runner orders files.
- **Description is freeform**: anything after the underscore is
  ignored by the runner; it surfaces in `pending()` rows for
  readability.
- **`.sql` extension required** for the up-migration; the down
  sibling appends `.down.sql` to the stem (the runner derives it
  by stripping `.sql` and appending).
- **Down siblings are optional** for `up`; required for `down`. If
  a `down` is requested and no `.down.sql` exists for a version
  being rolled back, `migrate_down` fails with
  `MigrationError { kind: "down_missing", ... }` and stops.

Files that don't match the `NNN_` prefix pattern (e.g. a README in
the migrations directory) are silently ignored. Files ending in
`.down.sql` are skipped during the up-iteration — they're loaded
on demand by `migrate_down`.

## Surface

The CONTRACTS.md surface, plus two documented deviations (see
`FRICTION.md`).

### Free-fn surface (the four CONTRACTS.md `Runner.*` methods,
translated to take a `Runner` first argument per the two-channel
rule):

```aperio
fn current_version(r: Runner) -> Int fallible(MigrationError);
fn pending(r: Runner)         -> db::Rows fallible(MigrationError);
fn migrate_up(r: Runner, target: Int)   -> Int fallible(MigrationError);
fn migrate_down(r: Runner, target: Int) -> Int fallible(MigrationError);
```

- `current_version(r)` — highest applied version, or 0 if none.
- `pending(r)` — newline-separated rows of `version\tfilename`
  for every up-migration with version > current.
- `migrate_up(r, target)` — apply pending migrations up to and
  including `target`. Pass `-1` to apply all. Returns the count
  of migrations applied.
- `migrate_down(r, target)` — roll back applied migrations down
  to (but not including) `target`. Pass `-1` for full rollback.
  Returns the count rolled back.

### Runner locus

```aperio
locus Runner {
    params {
        db_path: String = ":memory:";
        dir:     String = "migrations";
        ready:   Int    = 0;
        table:   String = "pond_migrations";
    }
    birth() { /* CREATE TABLE IF NOT EXISTS the tracking table */ }
}
```

The `db_path` field is the SQLite path (mirrored from `db::Db.path`);
the runner constructs its own Db handles inside the free-fn surface
because the v1 codegen rejects locus-typed `params` fields (see
`FRICTION.md`).

### MigrationError shape

```aperio
type MigrationError { kind: String; detail: String; version: Int; }
```

`kind` vocabulary: `"io"`, `"bad_filename"`, `"duplicate"`,
`"down_missing"`, `"exec_failed"`, `"version_table"`,
`"below_zero"`, `"above_latest"`. `version` is the migration the
runner was attempting when the failure fired (0 for pre-iteration
errors).

## CLI

The `migrate` binary (produced by `aperio build pond/migrations/`)
is the App-locus pattern wrapper around the free-fn surface.

```
$ migrate status                            # default: ":memory:", "migrations"
$ migrate status _ ./prod.db ./db/migrations
$ migrate up                                # apply all pending
$ migrate up 5                              # apply through version 5
$ migrate down 0                            # roll all back to version 0
$ migrate down 3                            # roll back to version 3
```

argv shape:

| Slot   | Meaning                            | Default       |
|--------|------------------------------------|---------------|
| `[1]`  | verb (`up` \| `down` \| `status`)  | `"status"`    |
| `[2]`  | target version (Int)               | `-1`          |
| `[3]`  | sqlite path                        | `":memory:"`  |
| `[4]`  | migrations directory               | `"migrations"`|

Positional defaults rather than `--flags` (the same choice
pond/subprocess made). When you need a different path with the
default verb / target, pass `_` (or any non-integer) as the
target — the CLI's argv parser only sets target if it parses as an
Int.

## Example

```bash
cd pond/migrations/examples/blog-schema
./blog-schema
```

The demo:

1. constructs a `Runner` against `:memory:` and the example dir
   (which holds `001_create_posts.sql` + `002_add_comments.sql`);
2. calls `migrate_up(r, -1)` to apply everything;
3. SELECTs `posts` and `comments` from `sqlite_master` to verify
   tables exist.

Today the demo prints `BLOCKED on 'exec_failed'` because db::exec
is stubbed. Post-unblock it prints `OK — both tables created`
without source change.

## Files

| File             | Role                                                   |
|------------------|--------------------------------------------------------|
| `errors.ap`      | `type MigrationError`                                  |
| `runner.ap`      | `locus Runner` (state + tracking-table birth)          |
| `migrate.ap`     | The four free fns + internal scan / apply helpers      |
| `cli.ap`         | `locus Migrate` App-locus + `fn main()`                |
| `examples/blog-schema/` | Two-migration end-to-end demo                   |

## Verification

```bash
aperio build \
    pond/migrations/
```

Type-checks and codegens cleanly. The demo additionally exercises
the full directory-scan → sort → iterate → exec pipeline against
the real fs surface; the SQL execution path is stubbed pending
sqlite's stdlib primitive.
