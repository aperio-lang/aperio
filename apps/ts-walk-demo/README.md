# ts-walk-demo

First dogfood of the **m96 `std::ts` (tree-sitter) substrate**.
Parses a Go source file and walks the tree, printing each named
node's grammar kind with depth-prefixed indentation.

This is the smallest end-to-end verification of the codebase-
onboarding substrate stack — it proves the chain
`Aperio source → std::ts path-call → lotus_ts_* extern → tree-
sitter / tree-sitter-go → handle-based AST → kind strings back
into Aperio` works in both directions.

## Run

```
aperio build apps/ts-walk-demo/main.ap
apps/ts-walk-demo/main apps/ts-walk-demo/sample.go
```

A different Go file path can be passed as `argv[1]`. Default is
`apps/ts-walk-demo/sample.go` so the demo runs cleanly from the
repo root.

> `aperio run` does not yet resolve qualified-name literals
> (`std::ts::*`), so this demo must be built rather than `run`'d
> for now. Tracked in `notes/aperio-friction.md`.

## What it shows

- `std::ts::parse_go(src) -> Int` returns a tree handle (1-based,
  0 on parse failure).
- `std::ts::root_node(tree) -> Int` returns a node handle.
- `std::ts::node_kind(n) -> String` returns the grammar kind name
  ("source_file", "function_declaration", "import_spec", ...).
- `std::ts::node_named_child_count(n) / node_named_child(n, i)`
  walk only the named-grammar nodes, skipping anonymous tokens
  like punctuation.

The sample Go file (`sample.go`) is small but exercises the
shapes m97's import-graph extractor will care about: package
clause, import spec, type declaration, method declaration,
function declaration. Output should look like:

```
(source_file)
  (package_clause)
    (package_identifier)
  (import_declaration)
    (import_spec)
      (interpreted_string_literal)
  (type_declaration)
    (type_spec)
      (type_identifier)
      (struct_type)
        ...
  (method_declaration)
    ...
  (function_declaration)
    ...
```

## Why this matters

Per `notes/codebase-onboarding-design.md`, the **import-graph
lotus** (m97) needs only tree-sitter — per-file import statement
extraction. Operational and domain extractors (m100, m102) layer
LSP and morpheme-rewriting on top, but the AST primitive sits
underneath them all. m96 + this demo are the substrate; m97
turns the substrate into a tower model.
