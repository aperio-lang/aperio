<!-- Synced from hale-lang/pond/math/matrix/README.md by tools/sync-pond-docs.sh — do not edit here. -->

# pond/math/matrix — dense row-major Float matrix

Suggested alias: `mat`.

```hale
import "vendor/pond/math/matrix" as mat;

let mx = mat::Mat { };
let a  = mx.from_rows(2, 3, "1, 2, 3, 4, 5, 6");
let i2 = mx.eye(2);
let z  = mx.zeros(3, 3);
let t  = a.transpose();                    // 3x2
let s  = mx.scale(a, 2.0);
let b  = mx.add(a, a);                     // 2x3
let c  = mx.matmul(i2, a);                 // 2x3 = I * A
let d  = mx.dot(a, a);                     // Float over flattened
```

## Storage

`Matrix` is a `@form(vec)` locus over `Float` cells with `rows` and
`cols` declared as required params. The form-synthesized `push`,
`get`, `set`, `pop`, `len`, `is_empty` methods cover the underlying
buffer. User-added methods on top:

| Method                      | Returns | Notes                                   |
|-----------------------------|---------|-----------------------------------------|
| `at(r: Int, c: Int)`        | `Float` | 0.0 on OOB; use `at_checked` for typed err |
| `set_at(r, c, v: Float)`    | —       | silent no-op on OOB                     |
| `transpose()`               | `Matrix` | fresh, heap-allocated                  |

Internal layout: row-major. Element `(r, c)` lives at flat index
`r * cols + c`. `data: heap of Float` carries `rows * cols` cells.
First `push` allocates a 4-element buffer; subsequent grows double
(per `spec/forms.md` § `@form(vec)` lowering).

## Namespace lotus — `Mat`

Factories + binary ops live as methods on a `Mat` namespace lotus
(empty params, methods only — pattern 2 in `spec/styleguide.md`).
See FRICTION.md for why this isn't the free-fn shape CONTRACTS.md
declared.

| Method                                       | Returns  | Failure shape              |
|----------------------------------------------|----------|----------------------------|
| `zeros(rows: Int, cols: Int)`                | `Matrix` | infallible                 |
| `eye(n: Int)`                                | `Matrix` | infallible                 |
| `from_rows(rows, cols, data: String)`        | `Matrix` | bad cells → 0.0 sentinel   |
| `matmul(a, b: Matrix)`                       | `Matrix` | shape mismatch → error sentinel (`is_error`) |
| `add(a, b: Matrix)`                          | `Matrix` | shape mismatch → error sentinel |
| `scale(a: Matrix, k: Float)`                 | `Matrix` | infallible                 |
| `dot(a, b: Matrix)`                          | `Float`  | shape mismatch → NaN (`is_nan`) |
| `error_matrix()`                             | `Matrix` | sentinel constructor       |
| `is_error(m: Matrix)`                        | `Bool`   | `m.rows < 0 \|\| m.cols < 0` |
| `nan_sentinel()`                             | `Float`  | IEEE 754 NaN               |
| `is_nan(f: Float)`                           | `Bool`   | `f != f`                   |

## Free-fn fallible surface

For agents that want the `or raise` / `or fallback` shape on
element access, top-level free fns carry the fallible(IndexError)
contract:

| Free fn                                                  | Returns                       |
|----------------------------------------------------------|-------------------------------|
| `index_of(rows, cols, r, c)`                             | `Int fallible(IndexError)`    |
| `at_checked(m, r, c)`                                    | `Float fallible(IndexError)`  |
| `set_at_checked(m, r, c, v)`                             | `Float fallible(IndexError)`  |

## Matmul complexity

The current `matmul` is the textbook three-nested-loop O(rows_a ·
cols_b · cols_a) algorithm. Per `spec/forms.md` performance bands,
the inner-loop primitives (`a.at`, `b.at`, `out.set_at`) hit:

- **Band (a) — tight-loop primitive (10% gate):** each `Matrix.get`
  / `Matrix.set` underneath the user-level `at` / `set_at`
  dispatches to `lotus_vec_get` / `lotus_vec_set` in the C runtime.
- **Band (b) — amortized (2× gate):** the per-element multiply-add
  mixes real arithmetic with the (a)-band primitives — the
  characteristic shape `spec/forms.md` benches against.
- **Band (c) — per-op fallible:** `Matrix.get` / `Matrix.set` are
  `fallible(IndexError)`; every call site addresses with `or 0.0`
  or `or discard`.

Future perf passes (tiling, SIMD via runtime extensions, or a
direct `lotus_vec_buf_ptr`-style escape hatch) can replace the
inner-loop body without changing the surface.

## Example

```bash
$ hale build pond/math/matrix/examples/matmul-demo/
$ ./pond/math/matrix/examples/matmul-demo/matmul-demo
A (3x3):
  1, 2, 3
  4, 5, 6
  7, 8, 9
I (3x3):
  1, 0, 0
  0, 1, 0
  0, 0, 1
I*A (3x3):
  1, 2, 3
  4, 5, 6
  7, 8, 9
...
```

Verifies I·A = A, computes A·A against hand-computed values, and
exercises the shape-mismatch sentinel paths for matmul and dot.

## Files

- `matrix.hl` — the `Matrix` locus, `Mat` namespace, free-fn
  fallible surface.
- `examples/matmul-demo/main.hl` — end-to-end demo.
- `FRICTION.md` — contract deviations, language gaps, duplication
  suspicions.
