# Mix pinned + cooperative threads

Aperio's concurrency model is bimodal: loci either share a
cooperative pool thread or own a pinned OS thread. The choice
is a *deployment seam* (F.31) declared in `main locus`'s
`placement { }` block — not on the locus declaration itself.
Cooperative loci yield to each other at substrate cells,
pinned loci run independently, and the bus crosses thread
boundaries through the standard mailbox+condvar machinery.

This recipe walks through the canonical "two loci on their
own threads publishing to a cooperative aggregator" shape.

## The placement block

```aperio
main locus App {
    params {
        agg:        Aggregator   = Aggregator { };
        worker_a:   WorkerA      = WorkerA { };
        worker_b:   WorkerB      = WorkerB { };
    }
    placement {
        // agg defaults to cooperative(pool = main)
        worker_a:    pinned;
        worker_b:    pinned(core = 3);
    }
}
```

- **Unspecified main-locus params** default to
  `cooperative(pool = main)` — `agg` doesn't need an entry.
- **`pinned`** gives the locus its own OS thread.
- **`pinned(core = N)`** additionally CPU-affinitizes the
  thread on Linux. On platforms without `sched_setaffinity`,
  the `core` arg is parsed and ignored.
- **Separate cooperative pools** — `cooperative(pool = io)`
  partitions cooperative loci onto a dedicated OS thread
  without going full pinned (useful when you want sibling
  isolation but not "own thread forever" semantics).

## A worked two-thread example

Two pinned workers publishing to a cooperative aggregator:

```aperio
type Sample { source: String; value: Int; }
topic Samples { payload: Sample; }

// --- Worker loci: bus-publishing only, no schedule on declaration. ---
locus WorkerA {
    bus { publish Samples; }
    run() {
        let mut i = 0;
        while i < 3 {
            Samples <- Sample { source: "A", value: i };
            std::time::sleep(80ms);
            i = i + 1;
        }
    }
}
locus WorkerB {
    bus { publish Samples; }
    run() {
        let mut i = 100;
        while i < 103 {
            Samples <- Sample { source: "B", value: i };
            std::time::sleep(50ms);
            i = i + 1;
        }
    }
}

// --- Aggregator: cooperative; receives from both. ---
locus Aggregator {
    params { count: Int = 0; }
    bus { subscribe Samples as on_sample; }
    fn on_sample(s: Sample) {
        self.count = self.count + 1;
        println("aggregator: got ", s.source, "/", to_string(s.value),
                " (total=", to_string(self.count), ")");
    }
}

// --- The deployment seam lives here. ---
main locus App {
    params {
        agg:       Aggregator = Aggregator { };
        worker_a:  WorkerA    = WorkerA { };
        worker_b:  WorkerB    = WorkerB { };
    }
    placement {
        worker_a:  pinned;
        worker_b:  pinned(core = 3);
    }
}

fn main() {
    App { };
}
```

The library code (WorkerA, WorkerB, Aggregator) carries no
placement decision — same loci could be deployed cooperative
or pinned in different binaries by varying `App`'s
`placement { }` block.

What runs where:

| Locus | Thread |
|---|---|
| `Aggregator` | main thread (pool `main`) |
| `WorkerA`    | its own pthread |
| `WorkerB`    | its own pthread (CPU 3) |

Output (timing-dependent on the exact interleave, but every
sample arrives at the aggregator):

```
aggregator: got B/100 (total=1)
aggregator: got A/0 (total=2)
aggregator: got B/101 (total=3)
aggregator: got A/1 (total=4)
...
```

## How the bus crosses threads

When a pinned locus publishes to a cooperative subscriber (or
vice versa), the runtime:

1. Resolves the subscriber's locus and notices its mailbox
   pointer (every pinned subscription gets a per-locus
   bounded ring buffer guarded by a mutex + condvar).
2. **Inline-copies the typed payload into a mailbox slot**
   before signaling — no shared-arena pointer crosses the
   thread boundary.
3. Broadcasts the condvar; the destination thread wakes,
   pops, copies the payload into **its own** arena, and
   invokes the handler.

The arenas stay single-threaded territory. Aperio commits to
"no shared mutable state across threads" structurally — the
two copies (publisher arena → mailbox → subscriber arena) are
the price.

## Placement tradeoffs

| You want | Reach for |
|---|---|
| The default. Almost everything. | `cooperative(pool = main)` (or no entry) |
| Latency-sensitive work (real-time ingest, tick handling) | `pinned` |
| Long-running CPU-bound loops that shouldn't yield | `pinned` |
| Predictable cadence on a specific CPU core | `pinned(core = N)` |
| Long-running cooperative sibling that shouldn't serialize against main | `cooperative(pool = own_pool)` |
| Anything else | cooperative on the default pool |

The rule of thumb: pinned is for *I shouldn't share a pool
thread*. If sharing is fine — even for a relatively-busy
locus — cooperative is the right answer. Use a separate
cooperative pool to partition siblings without going fully
pinned. The substrate yields between handler invocations and
between lifecycle transitions; you don't need pinned to "let
other loci run."

## What you can't do

- **No `greedy` placement class.** A placement that "shares a
  pool thread but never yields between handlers" would be a
  third class; the substrate refuses it. Cooperative already
  guarantees handler atomicity. If you don't want to yield
  between cells, you don't want to share a pool — use pinned.
  (Or use a separate cooperative pool with a single locus on
  it, which achieves "own thread" while staying on the
  cooperative side.)
- **No mid-handler yield in cooperative.** Within one handler
  body, the cooperative scheduler does not preempt. If you
  need to yield mid-work, factor into multiple handlers or
  use `std::time::sleep` / explicit `yield;`. (`time::sleep`
  folds in the cooperative bus drain after it returns, so a
  cooperative subscriber looping `while { sleep; ... }`
  delivers cross-thread bus traffic mid-loop without needing
  an explicit `yield;`.)
- **No shared mutable state.** No `Arc<Mutex<T>>`-shaped
  primitive. Cross-thread coordination is bus-shaped.

## Pinned-only `run()` at v1?

Earlier prototypes of pinned scheduling restricted pinned
loci to a single `run()` method (no bus, no other lifecycle
methods). The full pinned lifecycle — including the
cross-thread mailbox shown above — shipped as part of the
2026 substrate work and is the v1 surface. A pinned locus can
declare any combination of `birth` / `accept` / `run` /
`drain` / `dissolve`, plus `bus { subscribe ... publish ... }`.

## See also

- [Lifecycle & time](../concepts/lifecycle-time.md) — schedule
  classes, yield points, drain cascade.
- [The bus](../concepts/the-bus.md) §"Cross-thread bus
  semantics" — the concept-level treatment.
- [Run a topic across binaries](./multi-binary-bus.md) —
  pinned threading inside one binary is the in-process
  analogue of multi-binary deployment; same code shape, one
  scope wider.
