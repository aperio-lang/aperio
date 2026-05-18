<!-- Synced from aperio-lang/pond/trade/strategy/README.md by tools/sync-pond-docs.sh — do not edit here. -->

# pond/trade/strategy — strategy harness + PnL-attribution closure test

Suggested import alias: **`strat`**

```aperio
import "vendor/pond/trade/strategy" as strat;
```

A parent locus (`StrategyHarness`) that subscribes Tick + Quote
market data, accepts Strategy children (structural interface
F.20), and runs a **PnL-attribution closure test**: the sum of
child-reported PnL must match the harness's independently tracked
realized + unrealized books within a one-cent tolerance. If they
don't, the closure trips → `on_failure` → parent's policy.

This is the flagship demonstrator for **closure-test-as-PnL-audit**
(per the build assignment): the framework's `closure NAME { ... }`
primitive is exactly the right shape for "the strategy's books
must tie out with the executing layer's books." When they don't,
the substrate hands the discrepancy to the operator's policy
through `on_failure` — no panic, no silent loss, just a typed
`ClosureViolation` event with the closure name, epoch, left,
right, tolerance, and diff.

## Surface

```aperio
type Signal     { symbol: String; side: Side; strength: Float; ts: Time; }
type Position   { symbol: String; qty: Decimal; entry_price: Decimal; }
type PnlReport  { strategy_name: String; current_pnl: Decimal; ts: Time; }
type Fill       { symbol: String; side: Side; qty: Decimal; price: Decimal;
                  realized_pnl: Decimal; ts: Time; }
type PriceMark  { symbol: String; unrealized_pnl: Decimal; ts: Time; }
type Tick       { symbol: String; price: Decimal; qty: Decimal;
                  side: Side; ts: Time; }
type Quote      { symbol: String; bid: Decimal; ask: Decimal; ts: Time; }
type Side       { kind: String; }                       // "bid" | "ask"

interface Strategy {
    fn on_tick(t: Tick) -> ();
    fn on_quote(q: Quote) -> ();
    fn current_pnl() -> Decimal;
    fn current_positions() -> String;                   // tab-sep rows
}

locus StrategyHarness {
    params {
        name:           String;
        capital:        Decimal;
        realized_pnl:   Decimal;
        unrealized_pnl: Decimal;
        child_pnl_sum:  Decimal;
        child_count:    Int;
        child_names:    String;                         // newline-separated
        child_pnls:     String;                         // newline-separated
        tick_count:     Int;
        quote_count:    Int;
        fill_count:     Int;
        mark_count:     Int;
        report_count:   Int;
    }
    bus {
        subscribe "marketdata.tick"     as on_tick       of type Tick;
        subscribe "marketdata.quote"    as on_quote      of type Quote;
        subscribe "execution.fill"      as on_fill       of type Fill;
        subscribe "execution.mark"      as on_mark       of type PriceMark;
        subscribe "strategy.pnl_report" as on_pnl_report of type PnlReport;
    }
    closure pnl_balances {
        self.child_pnl_sum ~~ self.realized_pnl + self.unrealized_pnl
            within 0.01d;
        epoch dissolve;                                 // see Deviations
    }
    closure fatal_route { captures: name; epoch inline; }
}

topic SignalEvent     { payload: Signal;    subject: "strategy.signal";     }
topic PnlReportEvent  { payload: PnlReport; subject: "strategy.pnl_report"; }
```

## The closure-test pattern

The audit is a four-piece cycle:

```
                                 +-------------------------+
                                 |    StrategyHarness      |
        Tick / Quote ----------> |  (parent, books-side)   |
        ("marketdata.*")         |                         |
                                 |  realized + unrealized  |
        Fill / Mark -----------> |  (own books)            |
        ("execution.*")          |                         |
                                 |    closure              |
                                 |    pnl_balances:        |
                                 |    child_pnl_sum ~~     |
                                 |    realized+unrealized  |
                                 |    within 0.01d         |
                                 +------------^------------+
                                              |
                       PnlReport ("strategy.pnl_report")
                                              |
                                 +------------+------------+
                                 |     Strategy child      |
                                 | (structurally satisfies |
                                 |  the Strategy interface)|
                                 +-------------------------+
```

1. **Marketdata fan-in.** The harness subscribes
   `"marketdata.tick"` and `"marketdata.quote"` (literal-subject
   form per `pond/KNOWN_GOTCHAS.md` G1; cross-seed-by-name
   subscriptions don't survive codegen rewriting). Strategy
   children subscribe independently to the same subjects, so the
   harness's `on_tick` / `on_quote` handlers do bookkeeping only
   (count, last-seen timestamp). Routing happens at the bus, not
   in the harness body.

2. **Execution fan-in.** A separate execution layer publishes
   `Fill` (carrying a pre-computed realized_pnl delta) and
   `PriceMark` (carrying the current aggregate unrealized PnL).
   The harness updates `self.realized_pnl` / `self.unrealized_pnl`
   from these — its own books, independent of any strategy's view.

3. **Strategy report fan-in.** Each Strategy child publishes
   `PnlReport { strategy_name, current_pnl, ts }` on
   `"strategy.pnl_report"` whenever its books change. The harness
   upserts the per-strategy entry into a small (name → pnl) table
   and recomputes `self.child_pnl_sum`.

4. **The closure.** At its declared epoch (`dissolve` in v1; see
   Deviations), `pnl_balances` evaluates
   `child_pnl_sum ~~ realized_pnl + unrealized_pnl within 0.01d`.
   On a pass, the harness collapses cleanly. On a fail, the
   exploded flag is set; at end-of-life the parent of the harness
   receives `on_failure(harness, ClosureViolation { ... })` per
   F.9.

### When the closure fires

| Epoch       | Fires                                                                                                                    | Use case                                                       |
| ----------- | ------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------- |
| `dissolve`  | Once, at end of locus life (current v1 default).                                                                         | End-of-day audit; flush detect on shutdown.                    |
| `tick`      | Every bus handler invocation — produces false positives during in-flight (see Deviations).                               | Eventually-consistent monitor with a wide tolerance.           |
| `duration(d)` | Every `d` of monotonic time.                                                                                          | Mid-session sampled audit.                                     |
| `birth`     | Once, after birth.                                                                                                       | Initial-state invariants.                                      |
| `explicit`  | When user code calls `epoch_advance(pnl_balances)`.                                                                      | Manual settle: caller decides when state is consistent.         |
| `inline`    | Only via `violate pnl_balances;` — never auto.                                                                          | Pure structural-failure tag (no assertion).                    |

In v1 the harness ships `epoch dissolve` because the v1 `tick`
cadence (after every bus handler) trips on in-flight inconsistency
— see Deviations below for the rationale and the path forward.

## Deviations from `pond/CONTRACTS.md`

Surfaced while building this lib; full rationale in `FRICTION.md`.

1. **`epoch tick` → `epoch dissolve`.** The contract spells
   `epoch tick`; the implementation ships `epoch dissolve`. Per
   the 41-closure-accumulator fixture's documentation, `epoch
   tick` in v1 means "after every bus handler invocation" — a
   cadence that trips the closure on any in-flight inconsistency
   (a Fill arriving before its matching strategy PnlReport looks
   like drift until the second event lands). The audit's *intent*
   ("the books must tie") only makes sense once events have
   settled; `epoch dissolve` is the v1 expression of "books must
   tie by end-of-life."

2. **`sum(child.current_pnl)` → `self.child_pnl_sum` field.** The
   contract's closure assertion references
   `sum(child.current_pnl)` — sum over `self.children`, reading a
   method on each. Aperio v1's `sum(expr)` grammar primitive is a
   *streaming accumulator* over a single expression at each epoch
   fire (per the 41-closure-accumulator fixture); there is no
   for-comprehension form `sum(c.X for c in self.children)` in
   v1. The implementation moves the accumulation site from
   "inside the closure" to "inside the bus handler": each
   Strategy publishes its PnL on `"strategy.pnl_report"`; the
   harness updates `self.child_pnl_sum` keyed by `strategy_name`.
   The closure then references the field directly. Same audit
   cycle; only the accumulation moved one frame outward.

3. **`Strategy.current_positions() -> Rows` → `... -> String`.**
   The contract uses `Rows` from `pond/sqlite`. Pond's no-
   transitive-deps rule (`README.md` rule #4) means the strategy
   lib can't vendor sqlite. We return a structurally-equivalent
   `String` carrying newline-separated tab-separated fields,
   matching the wire form `sqlite::Rows` uses internally. A
   consumer that wants real `sqlite::Rows` converts at the call
   site.

4. **`topic Signal { payload: Signal; }` → `topic SignalEvent
   { ... }`.** Aperio's top-level declaration namespace doesn't
   distinguish `type X` from `topic X`; declaring both with the
   same name is a "duplicate top-level name" error. Same
   workaround `pond/trade/marketdata` uses for `TickEvent` /
   `QuoteEvent`. Wire subject stays `"strategy.signal"` so cross-
   seed subscribers using the literal-subject form aren't
   affected.

5. **Single-Strategy-per-harness in v1.** F.11 (`self.children`
   typing) ships with single-accept-type semantics at v0/v1; F.20
   Phase B follow-up notes that interface values can't yet sit in
   locus params/fields or be returned across arena boundaries.
   Together that means the harness can't hold a typed
   `children: [Strategy]` and call methods directly on them. The
   v1 routing path is the bus — each Strategy publishes
   `PnlReport`, harness aggregates by `strategy_name`, the
   closure reads the aggregate. The example demo wires one
   Strategy per harness for clarity; the harness's table
   structure supports N strategies at runtime (it's keyed by
   `strategy_name`).

## Two-channel rule

Per `AGENTS.md` / `spec/semantics.md` § "Where each channel
lives", user-declared locus methods cannot declare `fallible(E)`
(G4 in `pond/KNOWN_GOTCHAS.md`). Every method on
`StrategyHarness` is non-fallible:

- Operational errors (bad bus payload, unparseable PnL) would
  flow through the value channel via free-fn wrappers, or through
  the structural channel via `closure fatal_route { ... }` +
  `violate fatal_route;` (the styleguide §7 error-check-fn
  pattern). The `fatal_route` closure is declared and ready; v1
  has no reachable violation path because every handler degrades
  gracefully.
- Audit failures (the books don't tie) flow through `pnl_balances`
  → exploded flag → parent's `on_failure`. The audit IS a
  structural-channel event.

## Pattern catalog

| Locus / type        | Pattern                              |
|---------------------|--------------------------------------|
| `StrategyHarness`   | **Service locus** (catalog #3)       |
| `Strategy`          | **Structural interface** (F.20)      |
| `Signal`, `Position`, `PnlReport`, `Fill`, `PriceMark`, `Tick`, `Quote`, `Side` | **Shape type** (catalog #5) |
| `float_to_decimal`  | **Free fn** (catalog #6)             |

The harness is a Service locus with full lifecycle latent: no
explicit `run()` body because bus subscription handlers drive
every state change. Sentinel param (`name == ""`) lets `dissolve`
no-op on partially-constructed loci.

## Building

```
$ aperio build \
    pond/trade/strategy/
```

The lib itself typechecks; the build prints "program has no
`fn main()`" because a library has no entry point (same shape as
every other `pond/*` lib).

## Demo

```
$ aperio build \
    pond/trade/strategy/examples/momentum-strat/
$ pond/trade/strategy/examples/momentum-strat/main
```

The example runs two scenarios in the same process:

- **Scenario A (balanced):** `MomentumStrat` reports PnL in lock-
  step with the harness's books. At dissolve, `pnl_balances`
  passes silently.
- **Scenario B (deliberately broken):** a second `MomentumStrat`
  has `fudge_pnl: 1.50d` — it always reports $1.50 less than
  reality. At dissolve, `pnl_balances` trips because the books
  don't tie. Stderr shows the `ClosureViolation` line; process
  exits non-zero.

## Files

- `types.ap` — `Signal`, `Position`, `PnlReport`, `Fill`,
  `PriceMark`, `Tick`, `Quote`, `Side`.
- `interfaces.ap` — `Strategy` (F.20 structural interface).
- `topics.ap` — `SignalEvent`, `PnlReportEvent` (renamed to dodge
  the topic/type namespace collision; wire subjects unchanged).
- `harness.ap` — `StrategyHarness` locus + `float_to_decimal`
  helper.
- `examples/momentum-strat/main.ap` — runnable end-to-end demo.
- `FRICTION.md` — gaps, suspicions, deviations.
