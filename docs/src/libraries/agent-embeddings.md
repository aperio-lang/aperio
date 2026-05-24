<!-- Synced from hale-lang/pond/agent/embeddings/README.md by tools/sync-pond-docs.sh — do not edit here. -->

# pond/agent/embeddings — local embedding vector store with cosine top-k search

Suggested alias: `emb`.

```hale
import "vendor/pond/agent/embeddings" as emb;
import "vendor/pond/math/matrix"      as mat;

let mx    = mat::Mat { };
let store = emb::Store { dim: 4 };

// Insert a few vectors.
let v_red    = mx.from_rows(1, 4, "1.0, 0.0, 0.0, 0.0");
let v_red2   = mx.from_rows(1, 4, "2.0, 1.0, 0.0, 0.0");
let v_green  = mx.from_rows(1, 4, "0.0, 1.0, 0.0, 0.0");
store.add("red",   v_red,   "primary axis");
store.add("red2",  v_red2,  "near red");
store.add("green", v_green, "orthogonal");

// Top-k cosine search against a query.
let q    = mx.from_rows(1, 4, "3.0, 1.0, 0.0, 0.0");
let rows = store.search(q, 3);
println(rows.csv);
//   red2  0.989949  near red
//   red   0.948683  primary axis
//   green 0.316228  orthogonal
```

## Storage

`Store` is a user-declared locus that owns three child `@form(vec)`
sub-loci, paired row-by-row:

| Sub-locus | Cell type | Holds                                   |
|-----------|-----------|-----------------------------------------|
| `IdBuf`   | `String`  | one id per row                          |
| `MetaBuf` | `String`  | one metadata blob per row               |
| `FloatBuf`| `Float`   | flat `count * dim` floats, row-major    |

Row `r` occupies `flat[r*dim .. (r+1)*dim)`. The three buffers are
mutated in lockstep by `Store.add` / `Store.remove`.

CONTRACTS.md declared
`capacity { heap items of Embedding; }`, but `@form(vec)` cell types
must be primitives or value-typed structs — locus refs (the
`Matrix` field inside `Embedding`) are rejected at typecheck per
`spec/forms.md`. Parallel arrays are the v1 substitute. See
FRICTION.md.

## `Store` surface

| Method                                    | Returns | Failure shape                                     |
|-------------------------------------------|---------|---------------------------------------------------|
| `add(id, vector: Matrix, metadata)`       | —       | silent no-op on `vector.len() != dim`             |
| `search(query: Matrix, k: Int)`           | `Rows`  | empty `Rows` on dim mismatch / zero magnitude / `k <= 0` |
| `remove(id: String)`                      | —       | silent no-op when id not found                    |
| `count()`                                 | `Int`   | infallible                                        |

Methods are infallible per the two-channel rule
(`KNOWN_GOTCHAS G4` — locus methods can't declare `fallible(E)`).
Sentinel-substitute on bad input; the typed-error surface lives
in sibling free fns.

## Free-fn fallible surface

| Free fn                                                  | Returns                       |
|----------------------------------------------------------|-------------------------------|
| `add_checked(s: Store, id, vector: Matrix, metadata)`    | `Int fallible(EmbError)`      |
| `search_checked(s: Store, query: Matrix, k: Int)`        | `Rows fallible(EmbError)`     |
| `remove_checked(s: Store, id: String)`                   | `Int fallible(EmbError)`      |

```hale
let _ = emb::add_checked(store, "x", v, "meta") or self.handle(err);
```

`EmbError.kind` is one of `"dim_mismatch"`, `"bad_k"`,
`"zero_magnitude"`, `"not_found"`.

## Result shape — `Rows`

`Rows.csv` is newline-separated; each row is

```
<id>\t<score>\t<metadata>
```

`<score>` is a Float formatted via `to_string`; consumers can
round-trip via `std::str::parse_float`. Rows are sorted descending
by cosine score.

## Cosine similarity

Per the spec definition:

```
cosine(a, b) = dot(a, b) / (mag(a) * mag(b))
mag(v)       = sqrt(dot(v, v))
```

`dot` runs against the stored flat buffer in row-major order
(not via `mat::Mat.dot` on a per-row temporary — saves the per-row
Matrix allocation in the hot loop). Magnitudes are recomputed per
query rather than cached; the cache would invalidate on every
`add` / `remove` and the v0 inner-loop cost is already dominated
by the per-element `lotus_vec_get` boundary.

## Embedding type — `Embedding`

```hale
type Embedding {
    id:         String;
    vector_csv: String;
    metadata:   String;
}
```

CONTRACTS.md declared `vector: Matrix` (a locus ref). Cross-seed
`type` fields can't hold a qualified locus ref at v1 (codegen
crashes in pass A0; see FRICTION.md). The `vector_csv` field is
the row-major CSV serialization — round-trips via
`embedding_from_matrix` / `EmbeddingOps.to_matrix`. The Store's
surface doesn't consume `Embedding` directly — `add` takes the
Matrix straight through.

```hale
let e  = emb::embedding_from_matrix("id", v, "meta");
let eo = emb::EmbeddingOps { };
let m  = eo.to_matrix(e);
```

## Example

```bash
$ hale build pond/agent/embeddings/examples/topk-demo/
$ ./pond/agent/embeddings/examples/topk-demo/topk-demo
count = 5
--- top-3 ---
red2    0.989949    near red
red     0.948683    primary axis
green   0.316228    orthogonal
--- ranking ---
#1 = red2 (want red2)
#2 = red (want red)
#3 = green (want green)
ok   #1 = red2
ok   #2 = red
ok   #3 = green
...
```

Embeds 5 hand-crafted 4-dim vectors, queries with a 6th, verifies
the top-3 against by-inspection ordering. Exercises the dim-
mismatch `search_checked` path and the `remove` path.

## Files

- `embeddings.hl` — `Store`, parallel `@form(vec)` sub-loci,
  Embedding / SearchHit / Rows / EmbError shape types,
  `EmbeddingOps` namespace lotus, fallible free fns.
- `examples/topk-demo/main.hl` — end-to-end demo.
- `FRICTION.md` — contract deviations, language gaps,
  duplication suspicions.
