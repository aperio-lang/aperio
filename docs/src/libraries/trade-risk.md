<!-- Synced from aperio-lang/pond/trade/risk/README.md by tools/sync-pond-docs.sh — do not edit here. -->

# pond/trade/risk — risk gate locus

Suggested import alias: **`risk`**

```aperio
import "vendor/pond/trade/risk" as risk;
```

A single-locus risk-management gate that sits between a strategy's
`Signal` stream and downstream execution. Subscribes Signal from
`pond/trade/strategy`, applies position / drawdown / gross /
orders-per-second checks, and publishes `ApprovedOrder` for
signals that pass. Drawdown breaches fire **structurally** via a
closure-test cascade; the value-channel checks surface via
`self.last_breach`.

## Surface

```aperio
type Side  { kind: String; }                  // "bid" | "ask"
type Order { id: Int; side: Side; price: Decimal; qty: Decimal; ts: Time; }
type Signal { symbol: String; side: Side; strength: Float; ts: Time; }

type RiskLimits {
    max_position:          Decimal;
    max_drawdown:          Decimal;
    max_gross:             Decimal;
    max_orders_per_second: Int;
}
type RiskBreach { kind: String; symbol: String; detail: String; }

locus RiskGate {
    params {
        limits:           RiskLimits;
        current_drawdown: Decimal;
        // F.16 framework params (drive self.k_max):
        B: Int; c: Int; sigma: Int; phi: Float;
        // Internal state:
        last_breach:        RiskBreach;
        window_start_ns:    Int;
        orders_this_second: Int;
        current_position:   Decimal;
        gross_exposure:     Decimal;
    }
    bus {
        subscribe "strategy.signal" as gate of type Signal;
        publish   "risk.ApprovedOrder" of type Order;
    }

    closure within_limits {
        self.current_drawdown ~~ 0.0d within self.limits.max_drawdown;
        epoch tick;
    }

    fn check(s: Signal)            -> Bool;
    fn gate(s: Signal);
    fn set_drawdown(d: Decimal);
    fn effective_rate_cap()        -> Int;
}

topic ApprovedOrder { payload: Order; subject: "risk.ApprovedOrder"; }

// Free-fn wrappers + shielded reads (pattern 6):
fn check_or_raise(g: RiskGate, s: Signal) -> Bool fallible(RiskBreach);
fn effective_rate_cap(g: RiskGate) -> Int;
fn is_draining(g: RiskGate)        -> Bool;
```

> **Local mirror types.** `Side`, `Order`, and `Signal` are
> declared **locally** in `types.ap` as structural mirrors of
> `pond/trade/strategy::Side`/`Signal` and
> `pond/trade/orderbook::Order`. The v1 no-transitive-import rule
> (`spec/projects.md § "No transitive resolution"`) + the
> diamond-import duplicate-symbol error pinned this choice; the
> wire payload deserializes faithfully because bus serialization
> is shape-based per `spec/semantics.md § "Payload type"`. Same
> pattern strategy itself uses for `Side` / `Tick` / `Quote`.
> See `FRICTION.md` for the cross-pond context.

## Pattern catalog

`RiskGate` is a **Service locus** (pattern 3): full lifecycle is
latent (no explicit `run()` body; the locus is driven by the
`subscribe Signal as gate` bus binding). Sentinel params
(`current_drawdown == 0.0d`, `last_breach.kind == ""`) let
`dissolve` no-op on partially-constructed loci.

The free-fn wrappers in `ops.ap` are **pattern 6** (free fns
sitting at namespace level for consumer ergonomics — the
contract surface stays Bool-returning per CONTRACTS.md; the
wrapper re-encodes as `fallible(RiskBreach)` for callers that
prefer `or raise`).

## Closure shape — `within_limits`

```aperio
closure within_limits {
    self.current_drawdown ~~ 0.0d within self.limits.max_drawdown;
    epoch tick;
}
```

Per `spec/semantics.md § "Closure-test evaluation"`: at every
runtime tick boundary the runtime computes
`|self.current_drawdown - 0.0d|` and compares to
`self.limits.max_drawdown`. If the diff exceeds the band:

1. The locus's exploded flag flips and `self.draining` reads
   `true`.
2. A typed `ClosureViolation` event fires, carrying the closure
   name (`"within_limits"`) and the failing locus name.
3. The drain cascade initiates: any further publish-side bus
   sends from the locus are suppressed inside `check()`'s
   `self.draining` guard.
4. The runtime routes the violation to the parent's
   `on_failure(c: RiskGate, err: ClosureViolation)`. If no
   user-defined parent locus is in scope, the runtime root's
   default handler — `bubble(err)` — fires, which per
   spec/semantics.md § "Failure cascade" terminates the process
   with a structured exit (v1 runtime gap: see FRICTION.md).

A breach therefore "fires structural failure up to the parent's
on_failure," as the assignment requires. The value channel
(per-signal `check()` → Bool + `self.last_breach`) is orthogonal
— per the two-channel rule — and remains active for the
position / gross / rate refusals that don't trigger the drain.

## Limit checks — order of evaluation

Inside `check(s: Signal) -> Bool`, the gate evaluates (cheapest
first):

1. **Draining** — `if self.draining { refuse with kind="draining" }`.
   Per `spec/semantics.md § self.draining`: once the closure has
   fired (or any inline-violate), subsequent calls see the flag.
2. **Position** — `current_position + |qty| > max_position` →
   refuse with kind=`position`.
3. **Gross**    — `gross_exposure + |qty| > max_gross` →
   refuse with kind=`gross`.
4. **Rate**     — `orders_this_second >= effective_rate_cap` →
   refuse with kind=`rate`.

`effective_rate_cap` is `min(limits.max_orders_per_second,
floor(self.k_max))` (see "k_max as the rate ceiling" below).

On approval the gate increments aggregates, clears
`self.last_breach` to `kind == ""`, synthesizes an `Order` from
the Signal, and publishes `"risk.ApprovedOrder" <- o`.

## `k_max` as the orders-per-second ceiling (F.16)

Per `spec/design-rationale.md § F.16`, declaring the framework
parameters `B`, `c`, `sigma`, `phi` exposes `self.k_max` as a
built-in computed field:

```
self.k_max = B / [(1 - phi) * c + phi * sigma]
```

`RiskGate`'s defaults follow F.16's worked example
(`B=100, c=10, sigma=1, phi=0.5 ⇒ k_max ≈ 18.18`). The rate
check inlines `min(self.limits.max_orders_per_second,
floor(self.k_max))` — the tighter of the contract bound and
the framework's signature equation. Consumers retune the
ceiling by mutating any of the four params at runtime;
`self.k_max` re-evaluates per read.

**v1 codegen restriction.** `self.k_max` is only accessible
**inside locus method bodies**. External readers (free fns,
other loci, the demo) cannot do `g.k_max` — the codegen
synthetic-field lowering is gated on the `self.` receiver.
The `effective_rate_cap()` method exists for the external
read-shape but exposes only the static contract bound; the
real k_max-clipped ceiling is computed at the check site.
Logged in FRICTION.md.

## Two-channel rule

Per `AGENTS.md` and `spec/semantics.md § "Where each channel
lives"`: user-declared locus methods cannot declare
`fallible(E)`. CONTRACTS.md spells `check(s: Signal) -> Bool`
(already Bool-returning), so the gate matches the contract
without re-shaping. Refusal context flows through:

- **Value channel.** `self.last_breach: RiskBreach` is the
  per-call refusal payload. `kind == ""` after an approval;
  populated with one of `position` / `gross` / `rate` /
  `drawdown` / `draining` on refusal. The `check_or_raise`
  free fn in `ops.ap` re-wraps this as `fallible(RiskBreach)`
  for callers that prefer `or raise`.
- **Structural channel.** `closure within_limits { ...; epoch
  tick; }` audits `current_drawdown` against `max_drawdown`.
  Violation → bubble to parent → drain cascade.

## Bus topics

- `ApprovedOrder { payload: Order; subject: "risk.ApprovedOrder"; }`
  — every approved Signal yields one `ApprovedOrder` publish.
  The `subject:` field is pinned per KNOWN_GOTCHAS G1 (cross-seed
  topic-by-name publish/subscribe is broken at v1 codegen);
  cross-seed consumers subscribe via the literal-subject form
  `subscribe "risk.ApprovedOrder" as h of type LocalOrder;`.

## Building

```
$ aperio build \
    pond/trade/risk/
```

The lib itself reports `program has no fn main()` — that's the
expected library-shape outcome (CONTRACTS.md libs build via
their `examples/*` demos).

## Demo

```
$ aperio build \
    pond/trade/risk/examples/breach-demo/
$ pond/trade/risk/examples/breach-demo/main
```

Stands up a `RiskGate` with deliberately-tight limits, fires
Signals through a local `SignalFeeder` (publishes
`"strategy.signal"`); a sibling `ApprovedWatcher` subscribes
`"risk.ApprovedOrder"` to verify that breaching signals do NOT
emit an `ApprovedOrder`. Then primes the structural channel by
pushing `current_drawdown` past `max_drawdown` — the next
runtime tick fires the `within_limits` closure. See the demo's
in-source notes for the v1 runtime-bubble gap.

Expected output (abbreviated):

```
=== breach-demo: risk gate limit exercise ===
--- approving 3 unit signals via bus (within position cap) ---
  [ApprovedOrder #1] id=1 qty=1
  [ApprovedOrder #2] id=2 qty=1
  [ApprovedOrder #3] id=3 qty=1
after 3 approvals: position=3 gross=3
--- 4th signal via bus (still within cap) ---
  [ApprovedOrder #4] id=4 qty=1
--- 5th signal: strength 2.0 would push to 6.0 > cap; expect refusal ---
refused (position cap): kind=position symbol=GOOG — would exceed max_position
=== value-channel summary (BEFORE structural breach) ===
approvals seen by watcher: 4 (expected 4)
--- structural breach: set current_drawdown past max_drawdown ---
current_drawdown=2.5 (max_drawdown=1)
```

The watcher sees 4 ApprovedOrders for the 4 approved signals;
the 2 refused signals never publish. The structural breach
primes the closure for the next tick.

## Files

- `types.ap` — `Side`, `Order`, `Signal` (local mirrors),
  `RiskLimits`, `RiskBreach`.
- `topics.ap` — `ApprovedOrder` documentation (decl lives in
  `risk_gate.ap` to keep payload + import + publisher in one
  translation unit).
- `risk_gate.ap` — `RiskGate` locus + `closure within_limits` +
  `ApprovedOrder` topic decl + bridge helpers
  (`float_to_decimal_qty`, `decimal_abs_add`, `float_floor_to_int`,
  `to_float`, `__mono_seconds`).
- `ops.ap` — `check_or_raise` / `last_breach` / `current_position`
  / `gross_exposure` / `effective_rate_cap` / `is_draining`.
- `examples/breach-demo/main.ap` — runnable end-to-end demo.
- `FRICTION.md` — gaps, suspicions, deviations.
