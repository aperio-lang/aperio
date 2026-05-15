# The two failure channels

> **α** — Why does Aperio have two separate failure
> mechanisms, and how do you choose between them?

Failure-handling is where most languages quietly accumulate
the largest amount of accidental complexity. Exceptions vs.
sentinels vs. error returns vs. `Result<T, E>` vs. panics —
many languages have several of these layered, with different
disciplines for when to use which, often in the same codebase.

Aperio carves the space cleanly into two **orthogonal**
channels, with strict rules about which is allowed where:

- **The structural channel** (`↑`): a locus's declared
  invariant breaks. The runtime constructs a typed event and
  routes it upward to the parent's `on_failure` handler.
  Recovery primitives (`restart`, `quarantine`, `bubble`,
  `dissolve`) decide what to do.
- **The value channel** (`fallible(E)`): an individual call
  can fail with a payload. The caller MUST address the error
  inline via an `or` clause before consuming the value.

There is **no** `panic`, no `assert`, no `try`/`catch`, no
implicitly-propagating exception system. The two channels
above cover every legitimate failure case; anything else
indicates a category error in the modeling.

## The structural channel

A locus has *commitments it must hold across its lifetime*.
Those commitments are declared in `closure` blocks:

```aperio
locus PnLAttribution {
    params { intent_pnl: Decimal = 0.00d; book_pnl: Decimal = 0.00d; }

    closure books_balance {
        self.intent_pnl ~~ self.book_pnl within 0.05d;
        epoch tick;
    }
}
```

The `~~` operator is *approximate equality within tolerance*.
The closure says: at each tick, my intent PnL and book PnL
must agree within five cents. The runtime evaluates the
expression at each declared epoch; if it holds, nothing
happens (closures are silent on success). If it doesn't, the
runtime constructs a typed `ClosureViolation` event and routes
it to the parent's `on_failure`:

```aperio
locus TradingDesk {
    accept(p: PnLAttribution) { /* ... */ }

    on_failure(p: PnLAttribution, err: Error) {
        match err {
            Error::ClosureViolation(v) -> {
                // err.closure is "books_balance"
                // err.left, err.right are the two values
                // err.tolerance is 0.05d
                // err.diff is left - right
                quarantine(p) for 60s;
            }
            _ -> bubble(err);
        }
    }
}
```

The parent's recovery options:

- **Absorb** — return from `on_failure` without calling any
  recovery primitive. The child's failure is treated as
  "noted, not propagating."
- **`restart(child)`** — dissolve the child and instantiate a
  fresh one with the same declared params.
- **`restart_in_place(child)`** — reset the child to
  post-birth state while preserving its arena.
- **`quarantine(child) for d`** — pause the child but
  preserve its state, optionally auto-restart after `d`.
- **`bubble(err)`** — pass the failure up to *this* locus's
  parent. Recursive propagation.
- **`dissolve(child)`** — force-dissolve the child.

If a failure bubbles all the way past the runtime root with
no handler absorbing, the process exits non-zero with a
structured violation report on stderr. That's the only way
the program "crashes" — and it's a deliberate, structured
event, not an unexpected exception.

This is Erlang's let-it-crash philosophy with one important
addition: the parent's policy is *typed* and *declared*. You
write the recovery rule next to the locus it applies to, and
it can be different for different child types. The runtime
enforces the state machine — a child can't be running and
quarantined at the same time, can't accept while draining,
etc.

## The value channel

Sometimes a function can fail in a way that's not a structural
event — just "this call didn't produce a value, here's why":

```aperio
fn parse_player_id(s: String) -> PlayerId fallible(ParseError) {
    if !std::str::can_parse_int(s) {
        fail ParseError { kind: "not_int", input: s };
    }
    return PlayerId { value: std::str::parse_int(s) };
}
```

A function declared `fallible(E)` returns *either* a value of
the success type or a `FallibleErr(E)` payload. The caller
**must address** the error — the typechecker rejects a bare
call result:

```aperio
let id = parse_player_id(input);     // ERROR: "error not addressed"
```

You address it with an **`or` clause**, in one of three motions:

```aperio
let id = parse_player_id(input) or raise;          // propagate up
let id = parse_player_id(input) or default_id();   // substitute
let id = parse_player_id(input) or handle(err);    // hand off
```

- **`or raise`** — propagate the error one frame up the
  *static call stack*. The enclosing function must itself be
  `fallible(E)` (with the same payload type or a compatible
  one) so the error has somewhere to go. This is the value
  channel's version of "let it propagate."
- **`or <expression>`** — substitute a fallback value of the
  success type. `err` is implicitly bound to the payload
  inside the fallback expression. The fallback can be a
  literal (`or 0`), an expression (`or default_id()`), or a
  call (`or handle(err)`).
- **The error's payload type is fully typed.** You don't need
  to downcast or pattern-match a generic Error; the
  `fallible(E)` declaration says exactly what shape the
  payload has.

Chains work right-associatively:

```aperio
let id = parse_player_id(input) or lookup_default() or raise;
```

Reads as: try parse; on failure, try `lookup_default()`; on
*that* failure, propagate up. Each `or` disposes one fallible
in turn, reducing the chain toward a non-fallible value.

The value channel is value-level. It propagates through the
*static call stack*, not the locus tower. Two functions that
both `fallible(ParseError)` and call each other share the
same payload type and pass it up the stack until something
addresses it.

## Where each channel lives

This is the rule that often surprises people coming from
other languages:

> **`fallible(E)` may be declared on free functions and on
> stdlib-synthesized `@form(...)` methods. It may NOT be
> declared on user-declared locus methods.**

Why the restriction? Because locus methods are
*substrate-facing*. They participate in the locus's lifecycle
— bus subscription handlers, mode projections, contract reads.
Failures at this layer are *structural events*, not
value-level errors. They belong on the closure-violation
channel, where the parent's `on_failure` is the policy
handler.

If a locus method needs to expose application-layer failure
semantics, it wraps a fallible free function:

```aperio
fn parse_message(b: Bytes) -> Message fallible(ParseError) { ... }

locus Reader {
    bus { subscribe Input as on_input; }
    fn on_input(b: Bytes) {
        let m = parse_message(b) or default_message();
        // ... handle m
    }
}
```

The typechecker enforces this. Trying to declare `fn ... ->
T fallible(E)` on a user locus method produces a focused
diagnostic naming the rule.

The reverse direction has a complementary rule: only stdlib-
synthesized form methods (`@form(vec).get`, `@form(vec).pop`,
`@form(hashmap).get`, `@form(hashmap).remove`,
`@form(ring_buffer).pop`) declare `fallible(E)`. These are
application-layer storage substrate, not lifecycle-bearing
loci, so the value channel fits.

## Why two channels and not one?

Languages that have only structural failure (Erlang) make
value-level errors awkward — you end up modeling "couldn't
parse this int" as a process crash, which is too heavy.
Languages that have only value failure (Rust, Go) make
*structural* errors awkward — invariant violations end up
sprinkled across every call site as `Result<T, Error>`
returns, which is too granular and loses the parent-policy-
oriented recovery model.

Aperio splits the concern: structural failure routes up the
locus tower with typed policy, and value failure routes up
the static call stack with required inline disposition. The
two never mix at intermediate frames; the only place they
meet is the implicit root boundary (where any unhandled
error of either kind ends the process).

In practice the rule of thumb is:

| Failure shape | Channel |
|---|---|
| "This invariant I declared broke" | structural (closure → on_failure) |
| "This individual call can fail and the caller should choose" | value (fallible(E)) |
| "Couldn't parse" / "key not found" / "out of bounds" | value |
| "Books don't balance" / "k_max exceeded" / "child wedged" | structural |

## No panic / assert

Aperio has no `panic(msg)`, no `assert(cond)`, no `throw`.
"Impossible state" becomes "a closure asserting the state is
possible" — and when it isn't, the runtime constructs the
typed violation and routes it up. "Bail from this function"
becomes either `or raise` (value channel) or "make this a
closure on the locus" (structural channel).

This isn't asceticism. It's that every legitimate use of
`panic` falls cleanly into one of the two channels above,
with better typing and better recovery shape than `panic`
itself provides.

## Next

The next chapter, [Lifecycle & time](./lifecycle-time.md),
covers how loci come into being, run, and dissolve — the
state machine the failure channels operate over.
