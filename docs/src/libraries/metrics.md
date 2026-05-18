<!-- Synced from aperio-lang/pond/metrics/README.md by tools/sync-pond-docs.sh — do not edit here. -->

# pond/metrics — Prometheus-format exposition

Counter / gauge / histogram metric primitives plus a
Prometheus-compatible text-format renderer and a
`std::http::Handler` mountpoint for `/metrics`.

## Suggested alias

```aperio
import "vendor/pond/metrics" as metrics;
```

The bare alias `metrics` matches `pond/CONTRACTS.md`'s
suggestion and the entry in `pond/README.md`.

## Dependence

This lib has a hard source dependency on `pond/math/matrix` —
`Registry.histogram(name, buckets, labels)` accepts a `Matrix`
of bucket upper bounds, so the import is needed even if your
app never registers a histogram. Vendor both libs into your
app; the v1 transitive-dep rule (`pond/README.md` § "Design
rules") makes that explicit.

## Surface

```aperio
type Labels        { kv: String; }                  // "k1=v1\tk2=v2"
type MetricEntry   { ... }                          // hashmap cell
type HistogramData { ... }                          // bucket-vec cell

// labels constructors (bare free fns)
fn labels_empty() -> Labels;
fn labels_one(k: String, v: String) -> Labels;
fn labels_two(k1, v1, k2, v2) -> Labels;
fn labels_append(l: Labels, k: String, v: String) -> Labels;

locus Registry {                                    // single instance per app
    params { namespace: String = "";
             store: MetricMap;
             histograms: HistogramList; }
    fn counter(name: String, labels: Labels) -> Counter;
    fn gauge(name: String, labels: Labels) -> Gauge;
    fn histogram(name: String, buckets: Matrix, labels: Labels) -> Histogram;
    fn render() -> String;                          // Prometheus text format
}

locus Counter   { fn inc() -> (); fn add(v: Float) -> (); }
locus Gauge     { fn set(v: Float) -> (); fn inc() -> (); fn dec() -> (); }
locus Histogram { fn observe(v: Float) -> (); }    // closure-guarded monotonicity

locus MetricsEndpoint {                             // std::http::Handler
    params { registry: Registry; }
    fn handle(req: Request) -> Response;
}
```

### Registry (pattern 3 — long-lived service-style locus)

The single per-app metrics hub. Holds two `@form(...)` slots
internally:

- `store: MetricMap` (`@form(hashmap)`) — keyed lookup of every
  counter / gauge / histogram series by composite `name{labels}`
  key.
- `histograms: HistogramList` (`@form(vec)`) — parallel storage
  for bucket bounds + cumulative counts + sum + observation
  count per registered histogram series.

Factories (`counter`, `gauge`, `histogram`) idempotently
register the series and return a thin handle locus. Per AGENTS.md's
two-channel rule (`spec/semantics.md § Where each channel
lives`) the handle methods (`Counter.inc`, `Gauge.set`, ...) are
infallible — failures during the `@form(...)`-synthesized
substrate calls are absorbed locally via `or` clauses; they
don't bubble up the structural channel.

### Counter / Gauge / Histogram (pattern-3 handle loci)

Thin handles returned by Registry factories. Each carries direct
references to the underlying storage slots (`store: MetricMap`,
plus `histograms: HistogramList` for `Histogram`) and routes
every mutation inline. State lives in those slots; the handles
themselves are stateless modulo their addressing fields. The
handle pattern passes slots-of-self rather than `self` because
the latter trips a codegen v0 issue at the method-returns-
locus-with-self-as-field site — see `FRICTION.md`.

### Histogram structural invariant

The `Histogram` locus declares an inline closure invariant:

```aperio
closure buckets_monotonic {
    captures: out_of_order;
    epoch inline;
}
```

After each `observe(v)`, the locus recomputes
`out_of_order = count of cumulative-bucket pairs where
counts[i+1] < counts[i]` and `violate buckets_monotonic;`s if
the count is non-zero. The substrate's cumulative-increment
shape (`observe` only bumps; never decrements) keeps the
invariant trivially true under normal use; the closure is the
audit-channel guarantee that no future code path violates it
without surfacing in the parent's `on_failure(c, err)`. Per
`spec/styleguide.md § 7. Error-check fn pattern` the captures
clause names `out_of_order` so the `ClosureViolation` payload
routed to the parent carries the violating count.

### Labels constructors

The labels-constructor vocabulary is bare free fns matching
CONTRACTS.md. (Pre-A3 / lotus-lang `f9068fa` this lived on a
`Lab` namespace lotus as a workaround for the cross-seed
non-fallible free-fn call gap; that lotus was deleted in pond
pass D3.)

```aperio
let labels = metrics::labels_one("method", "GET");
```

## Example

```aperio
import "vendor/pond/metrics" as metrics;
import "vendor/pond/math/matrix" as mat;

fn drive(reg: metrics::Registry, mx: mat::Mat) {
    let hits = reg.counter("http_requests_total",
        metrics::labels_one("method", "GET"));
    let mem  = reg.gauge("process_resident_memory_bytes",
        metrics::labels_empty());
    let dur  = reg.histogram(
        "http_request_duration_seconds",
        mx.from_rows(1, 4, "0.005, 0.05, 0.5, 1.0"),
        metrics::labels_one("method", "GET")
    );

    hits.inc();
    hits.add(2.0);
    mem.set(120000000.0);
    dur.observe(0.012);
    dur.observe(0.4);

    println(reg.render());
    // # TYPE myapp_http_requests_total counter
    // myapp_http_requests_total{method="GET"} 3
    // # TYPE myapp_process_resident_memory_bytes gauge
    // myapp_process_resident_memory_bytes 120000000
    // # TYPE myapp_http_request_duration_seconds histogram
    // myapp_http_request_duration_seconds_bucket{method="GET",le="0.005"} 0
    // myapp_http_request_duration_seconds_bucket{method="GET",le="0.05"} 1
    // myapp_http_request_duration_seconds_bucket{method="GET",le="0.5"} 2
    // myapp_http_request_duration_seconds_bucket{method="GET",le="1"} 2
    // myapp_http_request_duration_seconds_bucket{method="GET",le="+Inf"} 2
    // myapp_http_request_duration_seconds_sum{method="GET"} 0.412
    // myapp_http_request_duration_seconds_count{method="GET"} 2
}

fn main() {
    let store = metrics::MetricMap { };
    let hl    = metrics::HistogramList { };
    let reg   = metrics::Registry {
        namespace: "myapp", store: store, histograms: hl
    };
    let lab = metrics::Lab { };
    let mx  = mat::Mat { };
    drive(reg, lab, mx);

    // Serve /metrics over HTTP:
    // std::http::Server {
    //     port: 9090,
    //     handler: metrics::MetricsEndpoint { registry: reg }
    // };
}
```

## Files

- `metrics.ap` — the whole lib (single-file seed; see
  `FRICTION.md` for why cross-file resolution forced the
  consolidation).
- `examples/exposition-demo/main.ap` — registers counter +
  gauge + histogram, mutates them, renders, asserts the body
  contains the expected Prometheus-format lines.

## Verification

```bash
aperio build \
    pond/metrics/examples/exposition-demo/
pond/metrics/examples/exposition-demo/exposition-demo
```

Expected: a dump of the rendered Prometheus body followed by
`exposition-demo: all format checks passed`.

Building the lib alone (`aperio build pond/metrics/`) fails
with "program has no `fn main()`" per the v1 single-binary
seed model — same shape every other pond lib follows (see
`pond/math/stats/README.md`).
