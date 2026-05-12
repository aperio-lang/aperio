# market-book

An embeddable market-book library, lotus-first. Two
loci — `MdGatewayL` and `BookL` — together cover the
snapshot + incremental ingest path that every market-data
consumer starts from.

## Framing

The user's framing:

> i'm imagining we have a 'gateway' lotus that is responsible
> for managing the actual connectivity layer, and it might
> also contain normalization lotus. ... the book lotus is
> then the lotus layer needed by any application that wants
> to interact with cohesive market state. the book subscribes
> to whatever streams it needs to, it knows when it's stable
> because it's got no inflight updates and it's not crossed
> or whatever, and then via that closure and the nested arena
> mechanics, accessing it via the shared memory of the lotus
> should be by design.
>
> apps COULD embed the mdgatewayL, it'd be the in-process bus

That's the v0 surface. One declaration per lotus. The same
shape runs co-located in one binary (this demo) or split
across processes once `std::bus::tcp` is wired (the gateway's
publish surface stays identical; the book's subscribe surface
stays identical).

## Lotus inventory

- **`MdGatewayL`** (`gateway.ap`) — the peer machine lotus
  that owns connectivity. v0 ships
  `fn emit_synthetic_feed()` that drives the snapshot + delta
  path deterministically. Real venues bind into the same
  publish calls behind a `fn connect_and_pump(host, port)`.
  Publishes four typed subjects under `book.*`.
- **`BookL`** (`book.ap`) — the embeddable consumer. Subscribes
  to the same four subjects, maintains a sorted per-side price
  ladder (8 levels per side), exposes a query surface
  (`best_bid` / `best_ask` / `mid` / `spread` / `depth_*` /
  `find_*_qty` / `is_stable` / `crossed_raw`) and a contract
  that lets a parent locus poll the stability invariant
  without reaching into private state.
- **`Fixed`** (`fixed.ap`) — money math. Int with a fixed 10^10
  scale baked into every helper. Float-free arithmetic so
  rounding never bites; `fixed_to_string` for human-readable
  output.
- **`Level`** (`level.ap`) — one row of a book.
- **`SnapshotBeginMsg / SnapshotLevelMsg / SnapshotEndMsg /
  DeltaMsg`** (`messages.ap`) — the four typed bus payloads.

## Wire shape

```
"book.snapshot.begin"  SnapshotBeginMsg
"book.snapshot.level"  SnapshotLevelMsg   { side, price_raw, qty_raw }
"book.snapshot.end"    SnapshotEndMsg
"book.delta"           DeltaMsg           { side, price_raw, qty_raw }
```

Side encoding: 0 = bid, 1 = ask. A delta with `qty_raw == 0`
removes the level at that price; non-zero sets it (insert if
new, replace if existing).

Prices and quantities ride as raw Int (Fixed.raw at scale
10^10). The bus payload wants primitives at v0 codegen;
fixed.ap reconstitutes them into `Fixed` at handler entry.
Once nested struct payloads ship, the messages will carry
`price: Fixed, qty: Fixed` directly.

## How to run

From the repo root:

```
cargo build --release -p aperio-cli
target/release/aperio build apps/market-book/
apps/market-book/market-book
```

Exit 0 silent after `all market-book assertions passed` is
the pass condition. Assertion failures print
`ASSERTION FAILED: <label>` with expected / actual rows and
exit 1.

## What the demo proves

`main()` instantiates one `MdGatewayL` and two `BookL`s. The
gateway emits a synthetic feed (3 bids + 3 asks in a
snapshot, then two deltas — one update, one removal). After
a single `yield;` to drain the cooperative bus queue:

- Both books contain the same final state.
- Best bid = 100.00 (qty 1200 after the delta update).
- Best ask = 102.00 (101 was removed by the delta).
- Spread = 2.00, mid = 101.00.
- `crossed_raw() == 0`, `is_stable() == true`.

The point isn't the synthetic feed — it's that two
independent subscribers converge on identical state purely
through bus dispatch. Plug a real venue in behind the
gateway's `connect_and_pump` and the same convergence holds
across N application binaries.

## Architecture: peer-machine vs in-process

The gateway / book split is the relocation boundary. Two
deployments share the same Aperio source:

1. **In-process embedding (this demo).** Gateway and book
   live in one binary. The bus is cooperative; `yield;`
   drains it. Latency is single-digit microseconds; memory
   sits in one arena cascade.
2. **Cross-process / cross-host (next milestone).** Gateway
   binary subscribes to a venue's TCP feed, publishes onto
   `book.*` over `std::bus::tcp`. App binary subscribes to
   the same subjects and instantiates `BookL` exactly as
   here. The book code doesn't change.

The "lotus boundary is the unit of relocation" principle
lives here in source: every cross-process boundary in this
codebase is a `publish` ↔ `subscribe` pair, never a function
call across machines.

## Stability invariant

A `BookL` exposes `is_stable()` returning true iff:

- `in_snapshot == false` — no ongoing snapshot epoch (book
  isn't in flux mid-replace).
- `crossed_raw() == 0` — best bid is strictly less than best
  ask (or one/both sides are empty).

The intended closure form:

```
closure not_crossed {
    self.crossed ~~ 0 within 0;
    epoch tick;
}
```

is captured in `book.ap` but not yet enabled — a transient
mid-snapshot crossing (asks arriving before all bids do)
would fire spuriously without a gate that pauses the closure
inside the snapshot window. Once closure gating is in,
flipping it on turns every crossing into a contract
violation routed to a parent locus's `on_failure`. The
explicit `is_stable()` query covers the same surface for now.

## v0 constraints, captured in code

- **Bus payloads ride raw Int, not Fixed.** `m70` wire
  format is primitives + String only. fixed.ap reconstitutes
  `Fixed` at handler entry. Future widening swaps in
  `price: Fixed, qty: Fixed` directly.
- **Array mutation via local-copy / write-back.** Compound
  `self.<arr_field>[i] = x` is not lowered yet, so every
  array update does `let mut next = self.arr; next[i] = x;
  self.arr = next;`. When the compound-assignment path
  ships, the helper bodies in `book.ap` collapse to in-place
  updates.
- **`not_crossed` closure is shape-only.** Snapshot-window
  gating not yet available; enforced via explicit
  `is_stable()`.
- **8 levels per side, hardcoded.** Top-of-book apps fit
  comfortably; full depth wants generic array sizing.
- **`std::bus::tcp` not shipped.** Cross-process deployment
  waits for it.

Each of these is one or two milestone movements away. The
shape of the library is what it should be; the workarounds
are local.

## What this app teaches — Memory-Owner Architecture

market-book is the canonical worked example of **Memory-Owner
Architecture** (`moa/MOA.md`). The three loci map to the three
MOA roles cleanly:

- **`MdGatewayL`** = **recording memory-owner**. State: one Int
  seq counter. Publishes four subject families (`book.snapshot.*`
  and `book.delta`). Ingest: none — gateway is a pure egress
  publisher in v0. It *records* the synthetic feed it generates
  by stamping each event with a monotonically-increasing seq.
- **`BookL`** = **projection memory-owner**. State: 8-level price
  ladder per side, plus `in_snapshot` / `crossed` / `last_seq`
  / counts. Subscribes to the four `book.*` subjects. Ingest
  classifications above each subscribe line: three `transform`
  (fold ladder state), one `save` (flip in_snapshot). It
  *projects* the gateway's delta stream into a cohesive ladder
  view; downstream consumers see the result, not the log.
- **`main()`** = **orchestrator**. Holds no state. Routes argv,
  instantiates the three memory-owners, kicks the feed. Every
  assertion in `main()` reads through `BookL`'s contract
  surface, never directly into state.

Reading the source in MOA order: start with `gateway.ap`
(simplest memory-owner — recording shape), then `book.ap`
(projection memory-owner — the interesting case, where the
ladder-mutation logic lives), then `main.ap` (orchestrator —
how the two compose). The MOA header comment at the top of
each file names the role explicitly.

The F.22 `capacity { pool ... }` lift for BookL's ladder
arrays is documented inline in `book.ap` as a v1.x migration —
deferred because `Cell<T>` is opaque round-trip at v1 and
BookL's indexed-access pattern (`bid_prices[i] = px`) needs
the load/store-through-cell surface that lands when Map/Vec
stdlib ships.

## Cross-references

- `moa/MOA.md` — the Memory-Owner Architecture pattern this
  app realizes
- `moa/subjects.md` — bus subject conventions; `book.*` is an
  app-defined family but follows the recording-stream shape
- `apps/reload-demo/` — the model hot-swap pattern (paired
  with this demo: reload-demo is the kernel-overlay shipping
  path, market-book is the substrate it'd hot-swap models
  onto).
- `notes/aperio-friction.md` — friction encountered while
  writing this app (bus payload primitives-only,
  array-field-index assignment).
- `docs/src/std/roadmap.md` — "Future arc — Lotus harness for
  n-dim growth."
