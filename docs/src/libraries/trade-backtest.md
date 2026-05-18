<!-- Synced from aperio-lang/pond/trade/backtest/README.md by tools/sync-pond-docs.sh — do not edit here. -->

# pond/trade/backtest — replay harness

Suggested import alias: **`bt`**

```aperio
import "vendor/pond/trade/backtest" as bt;
```

A CSV-driven replay harness for backtesting trading strategies
against a recorded tick feed. The `Backtester` locus loads a
feed, drives a `Strategy`-satisfying locus tick-by-tick, polls
PnL after each tick to accumulate drawdown + Sharpe, and
publishes the result on `trade.bt.complete`.

## Surface

```aperio
type BacktestResult { trades: Int; pnl: Decimal; max_dd: Decimal;
                      sharpe: Float; }
type BtError        { kind: String; detail: String; }
type Tick           { symbol: String; price: Decimal; qty: Decimal;
                      side: Side; ts: Time; }
type Signal         { symbol: String; side: Side; strength: Float;
                      ts: Time; }
type Side           { kind: String; }                    // "bid" | "ask"

interface Strategy {
    fn on_tick(t: Tick);
    fn on_quote(q: Tick);
    fn current_pnl() -> Decimal;
    fn current_positions() -> String;
}

interface Risk {
    fn check(s: Signal) -> Bool;
    fn set_drawdown(d: Decimal);
    fn current_drawdown() -> Decimal;
}

locus Backtester {
    params {
        feed_path: String;
        strategy:  Strategy;       // interface (F.20)
        risk:      Risk;           // interface (F.20)
        symbol:    String;
        speed:     Int = 1000;     // 1000x realtime
        // Read after run() completes:
        result:    BacktestResult;
        last_error: BtError;
        mock_time_ns: Int;
        // (plus internal accumulators)
    }
    bus { publish "trade.bt.tick"     of type Tick;
          publish "trade.bt.complete" of type BacktestResult; }
    run();
    dissolve();
}

// Contract's fallible(BtError) wrapper (free fn, per G4):
fn run_checked(b: Backtester) -> BacktestResult fallible(BtError);

// FeedReader: namespace lotus for streaming CSV parse.
locus FeedReader {
    params { symbol: String; }
    fn load(path: String) -> Bool;
    fn row_count() -> Int;
    fn row_at(i: Int) -> Tick;     // returns Tick { symbol: "" } on bad row
}

// Metrics: namespace lotus for drawdown + sharpe maths.
locus Metrics {
    params { last_peak: Decimal; last_dd: Decimal; }
    fn update_dd_inline(prior_peak: Decimal, prior_dd: Decimal,
                        cur_pnl: Decimal);
    fn sharpe(sum_returns: Float, sum_sq: Float, n: Int) -> Float;
}
```

## Wiring book + strategy + risk into Backtester

A consumer assembles the four pieces at peer scope and lets the
bus tie them together. Per `interfaces.ap` § deviation #2 the
Backtester does NOT hold the OrderBook directly (locus refs
can't sit in another locus's params per spec/types.md
§ "Vertical-only flow as a typing rule"); the book lives at peer
scope and a sibling locus injects orders onto it from the bus.

```aperio
import "vendor/pond/trade/backtest"  as bt;
import "vendor/pond/trade/orderbook" as book;
import "vendor/pond/math/matrix"     as mat;  // transitively needed
                                              // for orderbook's bulk()

// 1. Strategy — any locus satisfying the bt::Strategy interface.
let strat = MyMomentumStrategy { ... };

// 2. Risk — any locus satisfying the bt::Risk interface (e.g.
//    pond/trade/risk::RiskGate satisfies it structurally).
let risk = MyRiskGate { ... };

// 3. OrderBook — peer scope to the Backtester. The harness
//    publishes trade.bt.tick on the bus; a TickToOrder adapter
//    locus (caller-supplied) subscribes that subject and
//    injects orders into the book via book::add_checked.
let bk = book::OrderBook {
    symbol: "GOOG", tick_size: 0.01d,
};

// 4. Backtester — kicks off the replay on its let-bind's birth.
let b = bt::Backtester {
    feed_path: "ticks.csv",
    strategy:  strat,
    risk:      risk,
    symbol:    "GOOG",
    speed:     1000,
};

// 5. Surface the result via the contract's fallible shape.
let result = bt::run_checked(b) or raise;

println("trades=", to_string(result.trades),
        " pnl=",    to_string(result.pnl),
        " max_dd=", to_string(result.max_dd),
        " sharpe=", to_string(result.sharpe));
```

## CSV feed format

v1 supports a simple header-prefixed CSV:

```
timestamp,price,qty,side
2026-05-16T12:00:00Z,170.00,100,bid
2026-05-16T12:00:01Z,170.05,50,ask
...
```

- The header line is required (the loader skips line 1 unconditionally).
- Fields are comma-separated; ASCII whitespace around each field is trimmed.
- `side` must be `"bid"` or `"ask"`; malformed rows are skipped (with a `[feed]` warning on stdout) and excluded from the trade count.
- `timestamp` is read as a string into the Tick's `ts` field, but the harness's ordering uses file position, not the timestamp (v0 `Time` is string-shaped per `spec/types.md` and there's no `std::time::parse_iso8601` yet — see FRICTION.md).
- Symbol is NOT in the CSV; it's supplied as the `Backtester.symbol` param and is the same for every row.
- Decimal columns route through `parse_float + float_to_decimal` (no `std::str::parse_decimal` in v1; FRICTION.md tracks the gap).

## Bus topics (G1 literal-subject form)

Per `pond/KNOWN_GOTCHAS.md` § G1, cross-seed topic-by-name
publish/subscribe is broken at codegen. We use literal-string
subjects on the publish-side and document the same shape for
subscribers:

| Subject | Payload | When |
|---|---|---|
| `trade.bt.tick` | `Tick` | once per non-skipped CSV row |
| `trade.bt.complete` | `BacktestResult` | once at end of run() |

Subscribe shape (in a downstream consumer):

```aperio
locus TickAdapter {
    bus { subscribe "trade.bt.tick" as on_tick of type bt::Tick; }
    fn on_tick(t: bt::Tick) { ... }
}
```

## Two-channel rule

Per `AGENTS.md` / `spec/semantics.md` § "Where each channel
lives", user-declared locus methods cannot declare `fallible(E)`.
CONTRACTS.md spells `run() -> BacktestResult fallible(BtError)`;
this impl matches the surface via the `run_checked` free-fn
wrapper in `ops.ap`. The value channel is `b.last_error: BtError`
(`kind == ""` on success); the structural channel is `closure
feed_unreadable { ... }` (latent at v1, in place for v2
parent-supervisor on_failure routing).

## Pattern catalog

`Backtester` is a **Service locus** (pattern 3) — full
lifecycle, sentinel param `feed_path == ""` for partial-construct
dissolve safety, member-fn `_drive()` factored out so `run()`'s
lifecycle body stays return-free. `FeedReader` and `Metrics` are
**namespace lotus** (pattern 2). The free-fn surface
(`run_checked`, plus helpers `parse_csv_row`, `nth_csv_field`,
etc.) is **pattern 6**.

## Building

```
$ aperio build \
    pond/trade/backtest/
```

(Expected output: `codegen error: ... program has no fn main()` —
the same shape every pond lib seed shows. Library seeds don't
ship a main; the verification is "no type / parse errors".)

## Demo

```
$ aperio build \
    pond/trade/backtest/examples/replay-demo/
$ pond/trade/backtest/examples/replay-demo/replay-demo
```

Loads `examples/replay-demo/ticks.csv` (10 lines: 1 header + 9
data rows), drives a `TrivialStrategy` through the harness with
a `PassThruRisk` gate, prints the result.

## Files

- `types.ap` — `BacktestResult`, `BtError`, local `Side`,
  `Tick`, `Signal` mirrors.
- `interfaces.ap` — `Strategy`, `Risk` structural interfaces.
- `topics.ap` — `BtTick`, `BtComplete` topic decls (with
  literal `subject:` fields).
- `feed.ap` — `FeedReader` namespace lotus + CSV parsing helpers.
- `metrics.ap` — `Metrics` namespace lotus (drawdown +
  Sharpe maths).
- `backtest.ap` — the `Backtester` locus (Service, pattern 3).
- `ops.ap` — `run_checked` fallible(BtError) wrapper.
- `examples/replay-demo/main.ap` — runnable end-to-end demo.
- `examples/replay-demo/ticks.csv` — sample feed (10 lines).
- `FRICTION.md` — gaps, suspicions, deviations.
