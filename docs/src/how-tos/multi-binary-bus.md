# Run a topic across binaries

[The bus](../concepts/the-bus.md) introduces topics as
deployment-bindable channels: the same `subscribe` /
`publish` code works whether the topic is delivered
in-process or over a transport. This recipe walks through the
actual mechanics — declaring topics in a shared seed, wiring
transports in `main locus { bindings { ... } }`, and running
two binaries that exchange messages.

## What ships in v1

- **`in_memory`** — same-binary cooperative queue. The default
  when a topic has no binding.
- **`unix("/path") : listen | connect`** — AF_UNIX
  framed-byte transport. **Working in v1.**
- **`tcp("host", port) : listen | connect`** — parses but
  unimplemented at the runtime layer; linking emits an error.
- **`nats("nats://...")`** — same; parses-but-unimplemented.

Unix sockets are the v1 cross-process transport. Two binaries
on one host coordinate via a `.sock` path.

## Layout

We'll wire two binaries that share a `Tick` topic. The
publisher publishes ticks; the subscriber prints them. The
shared topic + payload type live in a third seed both binaries
import.

```
beats/                       ← workspace root (cd here to build)
  shared/
    topics.ap                ← topic Tick { payload: TickPayload; }
  publisher/
    main.ap                  ← import "shared" as shared; main locus
  subscriber/
    main.ap                  ← import "shared" as shared; main locus
```

## The shared seed

```aperio
// beats/shared/topics.ap
type TickPayload { n: Int; label: String; }
topic Tick { payload: TickPayload; }
```

That's it. The topic decl is one place; both binaries see the
same wire shape because they compile from the same source.

## The publisher

```aperio
// beats/publisher/main.ap
import "shared" as shared;

locus Producer {
    bus { publish shared::Tick; }
    run() {
        let mut i = 1;
        while i <= 5 {
            shared::Tick <- shared::TickPayload {
                n: i,
                label: "pub"
            };
            std::time::sleep(100ms);
            i = i + 1;
        }
    }
}

main locus PublisherApp {
    bindings { Tick: unix("/tmp/beats.sock") : connect; }
    run() {
        Producer { };
    }
}
```

The `bindings` block is **only legal in a `main`-modified
locus** (`main locus PublisherApp`, not bare `locus
PublisherApp`). A non-main locus carrying `bindings { }` is a
parse error.

`connect` is the **client side** — it opens a write-side
transport to the listening peer.

## The subscriber

```aperio
// beats/subscriber/main.ap
import "shared" as shared;

locus Consumer {
    bus { subscribe shared::Tick as on_tick; }
    fn on_tick(t: shared::TickPayload) {
        println("got tick #", to_string(t.n), " from ", t.label);
    }
}

main locus SubscriberApp {
    bindings { Tick: unix("/tmp/beats.sock") : listen; }
    run() {
        Consumer { };
        std::time::sleep(2000ms);   // keep alive long enough to receive
    }
}
```

`listen` is the **server side** — at `main`'s prelude, the
runtime calls `lotus_bus_register_remote(...)`, which binds
the AF_UNIX socket and spawns a reader thread. Inbound payloads
flow into the locus's normal handler dispatch.

## Build and run

From `beats/`:

```sh
aperio build subscriber/
aperio build publisher/

./subscriber/subscriber &       # start the listener first
sleep 0.1                       # give it a moment to bind the socket

./publisher/publisher
```

Expected output (from the subscriber):

```
got tick #1 from pub
got tick #2 from pub
got tick #3 from pub
got tick #4 from pub
got tick #5 from pub
```

## Bundle-wide rules

The compiler enforces:

1. **At most one `main` locus per bundle.** Zero is fine (a
   classic `fn main()` shape is still legal); two main loci
   is a compile error.
2. **Each `bindings` entry's topic must be declared.** A
   binding for an undeclared topic name is a compile error.
3. **A topic may appear at most once across all bindings.**
4. **`bindings` only inside a `main`-modified locus.** Any
   other location is a parse error.

## What `unix(...)` actually wires

At program startup, `main`'s prelude calls
`lotus_bus_register_remote(subject, "unix:///tmp/beats.sock", role)`
where `role` is `listen` or `connect`. The C runtime:

- **`listen` side** — `bind()`s the socket, spawns a pthread
  reader, fans incoming framed payloads into the local
  subscriber set.
- **`connect` side** — opens a write fd lazily on first
  publish, sends length-prefixed frames.

Framing is length-prefix per topic (the bus transport
contract is "deliver one whole message" regardless of
transport). You don't see the frames — the substrate handles
them.

## When the topic is also bound, the closed-world opt is skipped

The compiler normally rewrites a topic that's used only
within one locus type into a direct method call (the
"closed-world optimization"). A bound topic is never
optimized — the binding may publish to remote subscribers
the compiler can't see. This means adding a binding is
*always* a real bus traversal; the optimization quietly
stops applying.

## Mixing in-process and remote subscribers

A topic can have a binding **and** in-process subscribers.
Inbound payloads from the socket and locally-published
payloads fan out to the same handler set:

```aperio
main locus App {
    bindings { Tick: unix("/tmp/beats.sock") : listen; }
    run() {
        Consumer { };                  // local subscriber
        OtherConsumer { };             // another local subscriber
        // Remote publishers writing to /tmp/beats.sock also reach both.
    }
}
```

## What about TCP and NATS?

The parser accepts `tcp("host", port) : listen|connect` and
`nats("nats://...", ...)` shapes, but the runtime doesn't
implement them yet — `aperio build` will error at link time.
When they ship, the `bindings` syntax stays the same; the
transport choice flips with no application-code change.

## See also

- [The bus](../concepts/the-bus.md) — concept-level treatment
  of topics, hierarchical subjects, and the vertical-flow
  reconciliation.
- [Project layout](./project-layout.md) — cross-seed imports
  and the `aperio build` flow.
- [Structured logging](./logging.md) — `log.**` is a
  hierarchical topic that can be bound the same way to ship
  log events to a centralized aggregator binary.
