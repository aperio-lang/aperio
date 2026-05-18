<!-- Synced from aperio-lang/pond/trade/orderbook/README.md by tools/sync-pond-docs.sh — do not edit here. -->

# pond/trade/orderbook — limit-order book

Suggested import alias: **`book`**

```aperio
import "vendor/pond/trade/orderbook" as book;
```

A single-symbol limit-order book with id-keyed lookup, three
mode-projections of the same kernel state, and a `MoaOwner`
satisfaction so the book's evolving state can be mirrored
through `pond/moa`.

This is the flagship demonstrator for projection classes (per
the build assignment): one storage core, three observation
granularities, three implementations chosen by the compiler
from one declaration.

## Surface

```aperio
type Side  { kind: String; }                    // "bid" | "ask"
type Order { id: Int; side: Side; price: Decimal; qty: Decimal; ts: Time; }
type Level { price: Decimal; qty: Decimal; order_count: Int; }
type Top   { bid: Level; ask: Level; }

@form(hashmap)
locus OrderBook : projection chunked {
    params {
        symbol: String;
        tick_size: Decimal;
        gen_count: Int;
        epoch_id: Int;
        last_error: BookError;
        cached_top: Top;
    }
    capacity { pool orders of Order indexed_by id; }
    bus { publish BookUpdate; publish BookTop; }

    fn add(o: Order)               -> ();
    fn cancel(id: Int)             -> ();
    fn modify(id: Int, q: Decimal) -> ();

    mode bulk()       -> Matrix;     // depth ladder, Nx3 [price, qty, side]
    mode harmonic()   -> Top;        // top-of-book
    mode resolution() -> Decimal;    // mid-price

    // Mode-trampoline fns — v1 codegen rejects external mode
    // calls (`b.bulk()` from another seed). FRICTION.md § codegen
    // gap. Consumers use these instead; same return types,
    // direct delegations.
    fn query_bulk() -> Matrix;
    fn query_top()  -> Top;
    fn query_mid()  -> Decimal;

    // MoaOwner interface satisfaction:
    fn version()           -> Version;
    fn snapshot_bytes()    -> Bytes;
    fn apply_delta(d: Bytes);
}

// Free-fn fallible wrappers — the contract's fallible(BookError)
// shape per CONTRACTS.md. Return Bool (true on success) rather
// than () because v1 codegen rejects `-> () fallible(E)` —
// FRICTION.md § codegen gap.
fn add_checked(b: OrderBook, o: Order)               -> Bool fallible(BookError);
fn cancel_checked(b: OrderBook, id: Int)             -> Bool fallible(BookError);
fn modify_checked(b: OrderBook, id: Int, q: Decimal) -> Bool fallible(BookError);
fn has_order(b: OrderBook, id: Int) -> Bool;
fn order_count(b: OrderBook)        -> Int;

topic BookUpdate { payload: Order; }
topic BookTop    { payload: Top;   }

type BookError { kind: String; detail: String; }
```

## Projection-class choice — `chunked`

Per `spec/memory.md` § "Per-projection-class allocation":

| Class | Resolution served | Fits OrderBook? |
|---|---|---|
| **Rich** | Named-child observation (N≈4-10). | No — books carry tens to thousands of orders; named-child is the wrong granularity. |
| **Chunked** | Chunk-level observation (N≈10-30 typical, scales well). Moderate-to-high churn supported. Per-coordinatee sub-regions freed wholesale on departure. | **Yes.** Orders arrive + depart frequently; the book mixes id-keyed lookup (cancel/modify) with iteration ladders (bulk/harmonic/resolution). |
| **Recognition** | Aggregate-only ("represent as histogram"). No per-child resolution. | No — we explicitly need per-order resolution for cancel + modify. Recognition would force the API into a population-summary shape. |

In v1 the `: projection chunked` annotation is **documentary**:
the OrderBook locus doesn't `accept` child loci (orders live
inside the `@form(hashmap)`'s `pool orders` slot, not as nested
loci). The annotation lights up the chunked allocator if v2
wires per-order audit-trail children. The annotation also serves
as a forward-compatible commitment — the chunked sub-region-
per-child discipline is the right shape if order children land,
and downstream consumers reading the locus's projection class
through perspective machinery will see the chosen resolution
band.

## Storage discipline — `@form(hashmap)`

Per `spec/forms.md` § `@form(hashmap)`, the form lowers
`pool orders of Order indexed_by id` to an open-addressing
hashtable with backward-shift deletion. Key extraction GEPs
`Order.id` at every `set` call site; lookup / has / remove use
the same key shape.

Performance: O(1) expected for add/cancel/modify (the per-order
work); O(N) for the depth-ladder + top-of-book modes (each
walks every resting order). The microbench protocol for
`@form(hashmap)` is documented in `spec/forms.md` § "Bench
protocol".

## Three mode-projections

Per `spec/design-rationale.md` § F.5 and `spec/memory.md`
§ "Mode-projections share the arena", the three modes operate
on the same locus state via the same arena. The compiler
verifies no write-conflict; reads cost reflects the chunked
class's per-coordinatee bookkeeping.

### `mode bulk() -> Matrix`

Depth ladder as an `Nx3` matrix from `pond/math/matrix`. One
row per resting Order: `{price, qty, side_tag}` where
`side_tag == +1.0` for bid, `-1.0` for ask. Rows sorted by
price descending.

The Matrix cell type is Float (`pond/math/matrix`'s cell type
per its contract), so the Decimal `price` / `qty` get rendered
as Float in this projection's output shape. Callers needing
full Decimal precision read individual orders via
`book::order_count` + the `@form(hashmap)` accessor surface, or
subscribe to `book::BookUpdate`.

### `mode harmonic() -> Top`

Top-of-book: best bid (highest bid price), best ask (lowest
ask price), each with aggregated qty and order count at that
exact tick. Empty side → zeroed `Level { 0d, 0d, 0 }`.

### `mode resolution() -> Decimal`

Mid-price scalar: `(best_bid + best_ask) / 2`. Collapses to the
single-sided best when one side is empty; returns `0d` when
both sides are empty.

## Two-channel rule

Per `AGENTS.md` (and `spec/semantics.md` § "Where each channel
lives"), **user-declared locus methods cannot declare
`fallible(E)`**. CONTRACTS.md declares `add` / `cancel` /
`modify` as `fallible(BookError)`; the implementation matches
the call-site shape but routes failure two ways:

- **Value channel.** `self.last_error: BookError` — readable
  after every locus-method call. Successful calls set
  `kind == ""`. The free-fn wrappers (`add_checked` /
  `cancel_checked` / `modify_checked`) in `ops.ap` re-wrap this
  as the contract's `fallible(BookError)` for consumers that
  prefer `or raise` / `or handler(err)`.
- **Structural channel.** `closure fatal_op { captures:
  last_error, symbol; epoch inline; }` + a member fn that
  `violate fatal_op;` for unrecoverable corruption. Currently
  unreachable from the v1 surface (every operational error is
  recoverable); the shape is in place for v2 invariant checks
  (e.g., qty conservation across modify).

The deviation from CONTRACTS.md's signature literal is the same
deviation `pond/subprocess` and `pond/moa` resolved when they
hit the rule; see `FRICTION.md` for the cross-pond pattern.

## MoaOwner satisfaction

`OrderBook` implements the structural `pond/moa::MoaOwner`
interface (per `pond/moa/interfaces.ap`):

```aperio
interface MoaOwner {
    fn version() -> Version;
    fn snapshot_bytes() -> Bytes;
    fn apply_delta(d: Bytes) -> ();
}
```

All three methods are non-fallible (moa already resolved the
two-channel deviation at the interface level). Structural
satisfaction means no `impl I for L` is required — Aperio's
typechecker finds the three methods by name, arity, and return
type at the call site.

**Note on type spelling.** `orderbook.ap` declares a *local*
`type Version { generation: Int; era: Int; }` rather than
importing `pond/moa::Version`, because v1 codegen rejects
qualified type names in method signatures (FRICTION.md §
codegen gap). The local Version has field names and order
exactly matching `pond/moa::Version`, so the structural
interface check passes when a consumer imports both libs.
Same workaround `pond/agent/conversation` documented.

### Wire format

The v1 implementation uses a tab-separated single-line wire
format (owner-local per the moa contract):

```
snapshot:  "symbol\tepoch_id\tgen_count\n"        (header)
           "id\tside\tprice\tqty\n"               (per resting order)

delta:     "add\tid\tside\tprice\tqty\n"
           "modify\tid\tnew_qty\n"
           "cancel\tid\n"
```

Production owners that need denser framing (length-prefixed
binary, protobuf, CBOR) substitute their own owner — the wire
shape is intentionally outside the contract.

## Bus topics

- `BookUpdate { payload: Order; }` — every mutation fires
  exactly one `BookUpdate <- order;`. For `cancel`, the payload
  carries the order's last-known state (faithful to resting
  state at the time of departure).
- `BookTop { payload: Top; }` — fires after every mutation
  that **moves** the cached top of either side. The
  `maybe_publish_top` helper compares field-by-field with the
  cached `Top` and only publishes on change. Time-based
  throttling is the consumer's job (debounce / coalesce
  upstream of the subscriber); see `FRICTION.md` for the
  primitive gap.

## Pattern catalog

OrderBook is a **Service locus** (pattern 3) — full lifecycle
is latent (no explicit `run()` body; the locus is driven by
external calls). Sentinel `symbol == ""` lets `dissolve` no-op
on partially-constructed loci. The `add` / `cancel` / `modify`
surface is the operational locus method set; modes + MoaOwner
methods sit alongside.

The free-fn wrappers in `ops.ap` are **pattern 6** (free fns
not naturally hosted on the locus's method surface; the
fallible channel is the rationale).

## Building

```
$ aperio build \
    pond/trade/orderbook/
```

## Demo

```
$ aperio build \
    pond/trade/orderbook/examples/book-demo/
$ pond/trade/orderbook/examples/book-demo/main
```

Adds 10 orders on both sides of GOOG, queries top/depth/mid
before and after canceling a few, prints the BookUpdate /
BookTop publishes routed through a subscriber locus.

## Files

- `types.ap` — `Side`, `Order`, `Level`, `Top`, `BookError`.
- `topics.ap` — `BookUpdate`, `BookTop`.
- `orderbook.ap` — the `OrderBook` locus + helper free fns.
- `ops.ap` — `add_checked` / `cancel_checked` / `modify_checked`
  fallible wrappers + the consumer-facing helpers.
- `examples/book-demo/main.ap` — runnable end-to-end demo.
- `FRICTION.md` — gaps, suspicions, deviations.
