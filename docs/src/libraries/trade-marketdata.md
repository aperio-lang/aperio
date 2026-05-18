<!-- Synced from aperio-lang/pond/trade/marketdata/README.md by tools/sync-pond-docs.sh — do not edit here. -->

# pond/trade/marketdata — market data feed parsers + publishers

Suggested import alias: **`md`**

```aperio
import "vendor/pond/trade/marketdata" as md;
```

A pair of Service loci that emit `Tick` and `Quote` payloads on
the bus:

- `SyntheticFeed` — deterministic PRNG-driven feed for demos and
  backtests. Emits a `Tick` + a bracketing `Quote` at the
  configured cadence.
- `ItchParser` — Nasdaq ITCH 5.0 binary frame decoder.
  Pump-driven via `parse_chunk_step(b: Bytes)`, or one-shot via
  `source: <path>` + `run()` reading the whole file via
  `std::io::fs::read_bytes`.

Both publish on **literal subjects** (`"marketdata.tick"`,
`"marketdata.quote"`) because the cross-seed topic-by-name
publish/subscribe path is broken in v1 (KNOWN_GOTCHAS G1). The
topic decls in `topics.ap` declare matching `subject:` strings so
the wire-name story is consistent.

## Surface

| Name | Shape |
|---|---|
| `Side`        | type `{ kind: String }` ("bid" / "ask") |
| `Tick`        | type `{ symbol, price, qty, side, ts }` |
| `Quote`       | type `{ symbol, bid, ask, ts }` |
| `MdError`     | type `{ kind, detail }` (fallible payload) |
| `SyntheticFeed` | locus — service pattern; publishes Tick + Quote at `tick_interval` |
| `ItchParser`  | locus — service pattern; ITCH 5.0 frame decoder |
| `TickEvent`   | topic, `payload: Tick`,  `subject: "marketdata.tick"` |
| `QuoteEvent`  | topic, `payload: Quote`, `subject: "marketdata.quote"` |
| `parse_add_order(b, off, len)` | free fn fallible(MdError) — 'A' frame |
| `parse_trade(b, off, len)`     | free fn fallible(MdError) — 'P' frame |
| `parse_executed(b, off, len)`  | free fn fallible(MdError) — 'E' frame |

## Quick start — SyntheticFeed

```aperio
import "vendor/pond/trade/marketdata" as md;

locus Printer {
    bus { subscribe "marketdata.tick" as on_tick of type md::Tick; }
    fn on_tick(t: md::Tick) {
        println(t.symbol, " ", t.side.kind, " @ ", to_string(t.price));
    }
}

fn main() {
    let p = Printer { };                    // subscribers first
    let f = md::SyntheticFeed {
        symbol:        "AAPL",
        rate_per_s:    10,
        tick_interval: 100ms,                // load-bearing knob
        max_ticks:     50,
    };
    let _ = f;
    let _ = p;
}
```

The `examples/synth-feed-demo/` directory contains the runnable
version of this shape — 50 ticks at 10/sec, then `exit(0)`.

```
$ aperio build \
    pond/trade/marketdata/examples/synth-feed-demo/
$ pond/trade/marketdata/examples/synth-feed-demo/main
```

## Quick start — ItchParser

```aperio
import "vendor/pond/trade/marketdata" as md;

locus FeedConsumer {
    bus { subscribe "marketdata.tick" as on_tick of type md::Tick; }
    fn on_tick(t: md::Tick) { /* ... */ }
}

fn main() {
    let c = FeedConsumer { };
    // Pump-driven: caller feeds chunks of pre-framed ITCH bytes.
    let p = md::ItchParser { };
    let chunk: Bytes = /* read from your source */;
    p.parse_chunk_step(chunk);
    if p.last_error.kind != "" {
        println("itch err: ", p.last_error.kind, " — ", p.last_error.detail);
    }
    let _ = c;

    // Or: one-shot file mode (sets source, run() reads + parses).
    let p2 = md::ItchParser { source: "/path/to/itch.bin" };
    let _ = p2;
}
```

## Supported ITCH 5.0 message subset

| Type | Length | Behavior |
|---|---|---|
| `'A'` (65) — Add Order (no MPID)         | 36 bytes | emits `Quote` (bid or ask, side-keyed) |
| `'P'` (80) — Trade (non-cross)           | 44 bytes | emits `Tick` |
| `'E'` (69) — Order Executed              | 31 bytes | emits `Tick` (symbol/price empty — needs book layer) |

Other message types parse-skip cleanly via the length-prefix
framing. SoupBinTCP/MoldUDP 2-byte big-endian length prefix is
assumed at frame boundaries.

## Pattern selection

Both loci are Service loci (pattern 3) with full
`birth → run → dissolve` lifecycle. `SyntheticFeed.run()` drives
the tick loop; `ItchParser.run()` does a one-shot file read when
`source` is set, otherwise stays pump-driven for callers that
own the byte stream.

Per-frame parsers (`parse_trade` / `parse_add_order` /
`parse_executed`) are free fns (pattern 6) with
`fallible(MdError)` returns — the locus method
`parse_chunk_step` walks the buffer, dispatches into them, and
publishes the resulting Tick / Quote. The locus method itself is
non-fallible per the two-channel rule (KNOWN_GOTCHAS G4); the
fallible value channel lives on the per-frame free fns.

## Two-channel rule

CONTRACTS.md declares `ItchParser.parse_chunk(b) -> () fallible(MdError)`
on the locus. Per KNOWN_GOTCHAS:
- G4 (two-channel rule): locus methods can't carry `fallible(E)`.
- G2: `-> () fallible(E)` codegens an error regardless.

The implementation renames to `parse_chunk_step` (no fallible
marker) and surfaces errors via `self.last_error: MdError` —
`kind == ""` after a successful call. The per-frame free fns
ARE fallible; the locus method addresses their errors via
`or self.handle_parse_t(err)` / `_q(err)` error-check fns
(spec/styleguide.md § 7).

## Contract deviations

See `FRICTION.md` for the full set. Summary:

1. Topic names renamed `Tick` → `TickEvent`, `Quote` →
   `QuoteEvent`. CONTRACTS.md spells the topics with the same
   identifier as their payload type, which collides in Aperio's
   shared decl namespace.
2. `ItchParser.parse_chunk(b) -> () fallible(MdError)` renamed
   to non-fallible `parse_chunk_step` per G4 + G2.
3. `SyntheticFeed.tick_interval: Duration` added (CONTRACTS.md
   declares only `rate_per_s: Int`). `std::time::sleep` accepts
   `Duration` only, and `Int * Duration` arithmetic isn't
   surfaced in v1 — so the cadence has to come in pre-typed as
   a `Duration` literal (`100ms`, etc.). `rate_per_s` is
   retained for compatibility but is advisory.
4. `Side` redeclared locally (CONTRACTS.md spells it as
   `pond/trade/orderbook::Side`). Pond's no-transitive-deps
   rule means a `md` consumer shouldn't have to vendor
   `orderbook`. The shapes are structurally identical; a
   strategy that imports both libs converts via
   `Side { kind: t.side.kind }`.

## Building

```
$ aperio build \
    pond/trade/marketdata/
```

Type-checks cleanly. (The "no `fn main()`" diagnostic is the
expected "this is a library" signal.)
