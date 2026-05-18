<!-- Synced from aperio-lang/pond/math/stats/README.md by tools/sync-pond-docs.sh — do not edit here. -->

# pond/math/stats — descriptive statistics on Matrix inputs

Basic descriptive statistics plus a Welford running-moments locus.
Operates on the `Matrix` locus from `pond/math/matrix`.

## Suggested alias

```aperio
import "vendor/pond/math/matrix" as mat;
import "vendor/pond/math/stats"  as stats;
```

The bare alias `stats` matches `pond/CONTRACTS.md`'s suggestion and
the entries in `pond/README.md`.

## Dependence

This lib has a hard source dependency on `pond/math/matrix` — every
free fn here takes or returns `mat::Matrix`. Vendor both libs into
your app; the v1 transitive-dep rule (`pond/README.md` § "Design
rules") makes that explicit.

## Surface

```aperio
fn mean(xs: Matrix) -> Float;
fn variance(xs: Matrix) -> Float;
fn stddev(xs: Matrix) -> Float;
fn quantile(xs: Matrix, q: Float) -> Float fallible(StatsError);
fn min_max(xs: Matrix) -> Matrix;        // 1x2 [min, max]

locus OnlineMoments {                    // Welford running stats
    params { n: Int = 0; mean: Float = 0.0; m2: Float = 0.0; }
    fn observe(x: Float) -> ();
    fn current_mean() -> Float;
    fn current_var() -> Float;
}

type StatsError { kind: String; }        // "empty" | "out_of_range"
```

### Free fns (pattern 6)

- `mean(xs)` — arithmetic mean over the Matrix's storage-order
  elements. Empty input returns `0.0`.
- `variance(xs)` — population variance (`/N`, not `/(N-1)`). Empty
  input returns `0.0`. Two-pass: mean, then sum-of-squared-deviations.
- `stddev(xs)` — `std::math::sqrt(variance(xs))`.
- `quantile(xs, q)` — q-th quantile with linear interpolation
  between order statistics (the "type 7" / R-default convention).
  Fallible: `"empty"` for `xs.len() == 0`, `"out_of_range"` for
  `q < 0.0 || q > 1.0`. Copies into a 1xN scratch Matrix and sorts;
  does NOT mutate the input.
- `min_max(xs)` — returns a 1x2 Matrix `[min, max]`. Empty input
  returns `[0.0, 0.0]`.

All five operate on the Matrix as a flat sequence (storage order
via the `@form(vec)`-synthesized `get(i)`); shape — row-vec,
column-vec, full matrix — doesn't change the result.

### OnlineMoments (pattern 3, no lifecycle)

State-bearing locus with no lifecycle methods. One `observe(x)`
call per sample folds the new value into running mean and `m2`
(running sum of squared deviations) via Welford's algorithm in
O(1) per observation. `current_mean()` and `current_var()` read
off the running mean and population variance.

```aperio
let m = stats::OnlineMoments { };
m.observe(1.0);
m.observe(2.0);
m.observe(3.0);
let mu  = m.current_mean();   // 2.0
let var = m.current_var();    // 0.6666...  (population, /n)
```

Per AGENTS.md's two-channel rule, locus methods can't declare
`fallible(E)` — `observe` / `current_mean` / `current_var` are
infallible. Reading `current_mean()` / `current_var()` before any
`observe(...)` returns `0.0` (no error). That matches the
empty-as-zero convention the free fns use.

## Files

- `stats.ap` — the five free fns + `import "../matrix" as mat`.
- `online_moments.ap` — the `OnlineMoments` locus.
- `errors.ap` — `type StatsError`.
- `examples/stats-demo/main.ap` — small demo: batch mean+stddev,
  Welford on a stream, equivalence check.

## Verification

```bash
aperio build \
    pond/math/stats/
```

The library itself builds independently; the demo additionally
needs `pond/math/matrix` to be built — see FRICTION.md if the
sibling lib lags.
