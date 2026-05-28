# Mix pinned + cooperative threads

Hale's concurrency model is bimodal: loci either share a
cooperative pool thread or own a pinned OS thread. The choice
is a *deployment seam* (F.31) declared in `main locus`'s
`placement { }` block — not on the locus declaration itself.
Cooperative loci yield to each other at substrate cells,
pinned loci run independently, and the bus crosses thread
boundaries through the standard mailbox+condvar machinery.

This recipe walks through the canonical "two loci on their
own threads publishing to a cooperative aggregator" shape.

## The placement block

```hale
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

```hale
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

The arenas stay single-threaded territory. Hale commits to
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
| **Many concurrent I/O-bound connections on one thread** | `cooperative(pool = X) where async_io` |
| Anything else | cooperative on the default pool |

## Green-I/O cooperative pools (`where async_io`)

A cooperative pool's worker thread normally runs each handler to
completion before draining the next cell. That serializes
correctly for short-lived handlers but caps concurrent I/O at
one-blocking-call-per-pool-thread: a `recv_bytes` waiting on a
socket holds the worker until data arrives, so a second
connection's handler queues behind it.

A placement entry may declare `where async_io` to opt the pool
into green-I/O scheduling:

```hale
placement {
    listener: cooperative(pool = ws_accept)  where async_io;
    worker:   cooperative(pool = ws_workers) where async_io;
}
```

The pool's worker drain integrates an epoll instance. Inside a
locus method on this pool, blocking I/O syscalls (`recv_bytes`,
`accept_one`, `send_bytes`, `recv_str`, `send_str`) park the
calling coro instead of blocking the OS thread — the worker
runs other cells and other parked coros' wakeups until epoll
signals the parked fd is ready, then resumes the original coro.

User code stays synchronous-shaped:

```hale
locus PerConn {
    params { fd: Int = -1; }
    run() {
        let stream = std::io::tcp::Stream { conn_fd: self.fd };
        while true {
            let frame = stream.recv_bytes(4096);   // parks transparently
            if len(frame) == 0 { break; }
            handle_frame(frame);
        }
    }
}
```

Each `recv_bytes` call looks identical to its blocking-pool
counterpart; the substrate picks the right lowering at the
syscall boundary. N concurrent connections share one OS thread,
roughly 70 KiB per connection (64 KiB coro stack + per-conn
locus arena).

### Restrictions

- All placement entries on the same named cooperative pool must
  agree on `where async_io`. The drain loop is one-or-the-other.
- `where async_io` is rejected on `pinned` entries — pinned owns
  its own thread and has no shared drain loop to park on.
- `where async_io` is rejected on pool `main` — main runs inline
  on the binary's primary thread with no dedicated worker.

### Visibility

`std::process::dump_pool_residency()` writes one line per pool
to stderr with mode (async_io / blocking), parked-coro count,
and pending cell-queue depth. Call from a heartbeat tick on
long-running daemons.

See `spec/design-rationale.md § F.35` for the substrate design +
considered-and-rejected alternatives.

The rule of thumb: pinned is for *I shouldn't share a pool
thread*. If sharing is fine — even for a relatively-busy
locus — cooperative is the right answer. Use a separate
cooperative pool to partition siblings without going fully
pinned. The substrate yields between handler invocations and
between lifecycle transitions; you don't need pinned to "let
other loci run."

## What you can't do

- **No nested long-running cooperative children.** A non-main
  locus with a non-trivial `run()` body can't hold a `params`
  field whose declared type is a locus with its own non-trivial
  `run()` — including `std::http::Server` and the other entries
  on the substrate's known-long-running stdlib list. Nested
  children share the parent's OS thread, so the child's
  never-returning accept loop would starve the parent. The
  compiler rejects this at typecheck and points at the canonical
  fix: hoist both loci to siblings of a `main locus` and use a
  `placement { }` block to put them on different pools (the
  example in [The placement block](#the-placement-block) above
  is the canonical shape). See `spec/runtime.md § Long-running
  cooperative children`.
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
- **No shared mutable state, with one exception.** No
  `Arc<Mutex<T>>`-shaped primitive. Cross-thread coordination
  is bus-shaped — except when the shared state lives in a
  `@form(...)` cell layout. A locus declared `@form(hashmap)`
  / `@form(vec)` / `@form(ring_buffer)` is implicitly
  cross-pool-callable: the form ABI's cell accessors are the
  serializing layer, so producers and consumers on different
  pools can call its methods directly without going through
  the bus. The single-threaded-method invariant
  (`spec/types.md`) carves out this exception explicitly.
  Practical use: a Prometheus-style metrics registry held in
  `@form(hashmap)` of counter cells, incremented from
  multiple producer pools, rendered from one consumer pool.

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
