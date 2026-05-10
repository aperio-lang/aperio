# import-graph

**m97. The first lotus tower extractor.** Walks a directory of
Go files, parses each with `std::ts`, and emits a JSON tower
model — per-file `id`, `package`, and `imports` — to stdout.

This is the cheapest of the three lotus perspectives in
`notes/codebase-onboarding-design.md` (import-graph = harmonic
mode), shipped first to validate the extraction pipeline before
spending months on the visualization substrate.

## Run

```
aperio build apps/import-graph/main.ap
apps/import-graph/main apps/import-graph/fixture
```

`argv[1]` is the directory to scan; defaults to
`apps/import-graph/fixture/` so the demo runs from the repo root
with no flags. Output goes to stdout.

> `aperio run` does not yet accept qualified-name literals
> (`std::ts::*`, `std::io::fs::*`), so this app must be built
> rather than `run`'d. Tracked in `notes/aperio-friction.md`.

## Output shape

```json
{
  "dir": "apps/import-graph/fixture",
  "files": [
    {"id": "greet.go",  "package": "main", "imports": ["fmt", "strings"]},
    {"id": "util.go",   "package": "main", "imports": []},
    {"id": "server.go", "package": "main", "imports": ["net/http", "strconv"]},
    {"id": "main.go",   "package": "main", "imports": ["fmt"]}
  ]
}
```

Tower entries are *files*, not packages. Every `.go` file in the
scanned directory becomes one entry. Files with no imports keep
the field present as an empty array — downstream consumers can
rely on the shape.

## What it shows

This is the **harmonic-mode** view of the codebase: structural
relationships across files, derived purely from tree-sitter (no
type info needed). Per the design doc, harmonic-mode answers
*"what depends on what?"* — and that's exactly what an import
graph is.

The extractor is ~150 lines of Aperio. It composes:

| Substrate                     | Use                          |
|-------------------------------|------------------------------|
| `std::io::fs::list_dir`       | Enumerate files in the dir   |
| `std::io::fs::read_file`      | Read each `.go` source       |
| `std::ts::parse_go`           | Parse to a tree-sitter tree  |
| `std::ts::node_named_child*`  | Walk the AST                 |
| `std::ts::node_kind`          | Identify import_declaration  |
| `std::ts::node_text`          | Extract import path strings  |
| `std::str::index_of` + slice  | Manual list_dir + import split |

All shipped surface — m97 added zero new substrate.

## v0 scope cuts

These show up in the friction log; addressing them is each its
own milestone:

- **File-level only.** Multiple `.go` files in one directory all
  belong to one `package main`, but the tower emits them as
  separate entries. Package-level aggregation (group by
  `package` field) is a downstream pass; the file-level shape
  preserves enough information.
- **Single directory only.** No recursion into subpackages —
  `list_dir` is non-recursive and Aperio doesn't yet ship a
  directory-tree walker. Real Go modules with nested package
  trees need either an `fs::walk` primitive or a recursive
  helper in Aperio source.
- **Aliased imports collapse.** `import f "fmt"` → `"fmt"`. The
  alias is dropped at v0; `interpreted_string_literal` is the
  only thing the extractor reads from `import_spec`.
- **No JSON escaping.** Go package names and import paths can't
  contain `"` or `\`, so unescaped concat is safe in practice.
  A general-purpose `std::json::*` is a future stdlib.
- **No cycle detection.** The tower is data; the F.4-violation
  check (per the design doc, Aperio doesn't permit cyclic
  dependencies) waits on a downstream consumer that walks the
  emitted tower.

## What's next

The tower model is data. Two natural follow-ups:

1. **m98 — `std::graphics`** plus **m99 — tower visualization
   v0.** Render this JSON as a 3D lotus shape; the smallest
   end-to-end "you see your codebase as a lotus" demo.
2. **m100 — operational extractor** (LSP-driven). Adds the
   second tower (resolution mode) — lifecycle objects, async
   tasks, entrypoints. Composes with `std::lsp` (m101).

Per the design doc, **m96 + m97 + m98 + m99 is the candidate
demo product**: one tower (this one) rendered with the graphics
substrate. m103 is the real product (three towers + mode
switching).
