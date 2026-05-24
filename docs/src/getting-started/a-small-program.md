# A small program with shape

`Greeter` shows what one locus looks like in isolation. Real
programs are more than one. Loci coordinate over a **typed bus**
— a publish/subscribe channel where subjects are first-class
declarations, not strings.

Here's a small program with three loci communicating over one
topic:

```hale
type Tick { n: Int; }
topic Beats { payload: Tick; }

locus Counter {
    params { sum: Int = 0; }
    bus { subscribe Beats as on_beat; }
    fn on_beat(t: Tick) { self.sum = self.sum + t.n; }
}

locus Echoer {
    bus { subscribe Beats as on_beat; }
    fn on_beat(t: Tick) { println("tick: ", t.n); }
}

locus Pulse {
    params { iters: Int = 4; }
    bus { publish Beats; }
    run() {
        let mut i = 1;
        while i <= self.iters {
            Beats <- Tick { n: i };
            i = i + 1;
        }
    }
}

fn main() {
    let c = Counter { };
    Echoer { };
    Pulse { iters: 4 };
    print("sum=");
    println(c.sum);
}
```

Save it as `beats.hl` and run:

```sh
hale run beats.hl
```

Output:

```
tick: 1
tick: 2
tick: 3
tick: 4
sum=10
```

## What's happening

Three loci, one topic, two subscribers.

- **`type Tick`** is a value-shape record. No lifecycle, no
  flow — pure data that crosses the bus.
- **`topic Beats`** names a typed channel carrying `Tick`
  values. The payload type travels with the declaration, not
  with each subscriber.
- **`Counter`** subscribes to `Beats`; its `on_beat` handler
  accumulates `t.n` into `self.sum`.
- **`Echoer`** subscribes to the same topic and prints each
  tick. Two subscribers, one topic, no coordination needed
  between them — the bus does fan-out invisibly.
- **`Pulse`** publishes four ticks, then exits its `run()`
  body.

Notice what's *not* in the program:

- No channel-creation boilerplate. The topic IS the channel.
- No subscriber-registration calls. The `bus { subscribe ... }`
  block IS the registration.
- No event-loop. The runtime drains pending bus events at
  cooperative yield points; `run()` and the handlers compose
  naturally.
- No coordination between `Counter` and `Echoer`. The fact
  that two loci listen to the same topic is not their concern;
  it's the bus's.

## Locus lifetimes here

Three different locus shapes get instantiated in `main`:

- **`let c = Counter { };`** — a *let-bound* locus. `c` is a
  handle to the locus; the binding stays valid for the rest of
  the function. `Counter` dissolves at the end of `main`.
- **`Echoer { };`** (no binding, but has bus subscriptions) —
  a *long-lived anonymous child*. Because `Echoer` has a bus
  subscription, the runtime keeps it alive past the statement
  boundary so it can still receive events. It dissolves at the
  end of `main` alongside `Counter`.
- **`Pulse { iters: 4 };`** (no binding, has `run()` but no
  subscriptions) — a statement-position literal with work to
  do. Its `run()` body fires synchronously, all four ticks
  flow through the bus, and `Pulse` dissolves at the statement
  boundary.

The pending bus events fire before `Pulse` dissolves, so by
the time `println(c.sum)` runs, both subscribers have
processed all four ticks.

## Where to next

This program already raises questions the **Concepts** chapters
answer:

- *What's the rule about who subscribes vs. who publishes?* —
  See [The bus](../concepts/the-bus.md).
- *Why does an anonymous `Echoer` stay alive but anonymous
  `Pulse` doesn't?* — See
  [Lifecycle & time](../concepts/lifecycle-time.md).
- *What's the right way to organize this program if there
  were ten subscribers, or if `Counter`'s state had to survive
  a restart?* — See
  [The locus](../concepts/the-locus.md) and
  [Modeling — how to think in Hale](../concepts/modeling.md).

The next section is [Concepts](../concepts/the-locus.md), which
walks through the structural model one primitive at a time.
