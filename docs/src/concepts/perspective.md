# Perspective & observation

> How does a locus expose a serializable view of
> itself across process boundaries?

A locus's state is private to its region. Its
contract-exposed surface is visible to its parent. But what
about an observer that *isn't* the parent? A separate
analytics binary that wants to read the locus's state? A
parameter-fitting service that produces values for a
parameter-applying service?

The `perspective` primitive is Hale's answer: a typed,
serializable view of a locus that can travel across
process boundaries — with the compile-time guarantee that
the producer and consumer share a schema, because they
compile from the same source.

This is the smallest chapter in Concepts because the
underlying machinery is small. Perspective is a sharp tool
used by a few specific designs (parameter-fitting,
hot-loading kernels, cross-process state propagation); most
locus authors will never declare a `perspective`.

## What a perspective declares

```hale
perspective Kernel {
    params {
        scale_row:   [Decimal; 8];
        sigma_factor: Decimal;
        regime_id:   Int;
    }
    stable_when {
        return self.num_validated >= 3 && self.closure_status == "ok";
    }
    serialize_as KernelV1;
}
```

Three pieces:

- **`params`** — a parameter bundle. Same shape as a locus's
  `params` block: typed fields with defaults or `: inferred`.
  This *is* the serialized payload — the schema is the type.
- **`stable_when`** — a boolean predicate the runtime
  evaluates to decide whether the perspective is "ready to
  ship." This is where multi-perspective stability lives in
  the source: the perspective tells the runtime, in its own
  voice, what conditions it must meet before being published.
- **`serialize_as TypeName`** — optional annotation declaring
  a stable name for the wire format (lets the perspective's
  identifier be renamed without breaking serialization).

A perspective is **not** a locus. It has no lifecycle, no
contract block, no bus interface, no methods beyond
`stable_when`. It's a typed parameter bundle the substrate
knows how to validate and ship.

## The fitter / applier pattern

The canonical use case is the *fitter/applier* split. Two
binaries:

```hale
// fitter.hl — observes inputs, fits Kernel parameters
perspective Kernel { /* ... as above ... */ }

topic KernelUpdates { payload: Kernel; }

locus Fitter {
    bus { publish KernelUpdates; }
    run() {
        let mut k = compute_kernel(observations);
        while !k.is_stable() {
            k = refine_kernel(k, more_observations());
        }
        KernelUpdates <- k;
    }
}
```

```hale
// applier.hl — applies the latest Kernel at high frequency
perspective Kernel { /* same declaration, same source */ }

topic KernelUpdates { payload: Kernel; }

locus Applier {
    params { current_kernel: Kernel = default_kernel(); }
    bus { subscribe KernelUpdates as on_update; }
    fn on_update(k: Kernel) {
        self.current_kernel = k;     // atomic swap; readers see consistent state
    }
    run() {
        // high-frequency loop using self.current_kernel
    }
}
```

Both binaries compile from the same `Kernel` perspective
declaration. The type *is* the protocol — there's no
schema-versioning handshake, no protocol-buffer regen step,
no risk of fitter and applier disagreeing about the shape.
If you change the perspective, both rebuild from the same
source.

The runtime guarantees the swap on the consumer side is
atomic: readers in the consumer locus see the pre-swap
perspective or the post-swap perspective, never a torn read.

## `stable_when` — multi-perspective stability

The `stable_when` predicate lets a perspective decline to
ship until it's earned the right. In the Kernel example, it
requires at least three independent validations and a passing
closure check before it'll be considered stable. The
publishing locus can check `k.is_stable()` (an implicit
method on every perspective) before deciding to publish.

The predicate is just a Bool-returning block. It can
reference `self` (the perspective's params), and free
functions in scope. The runtime evaluates it on demand —
typically before each potential publish, and once at the
consumer side after a candidate perspective is decoded but
before it's atomically installed.

This makes "perspective is ready to ship" a *property of the
data* declared in the data's own type, not a flag in the
publisher's code or an off-by-default config. It's stable
when it says it's stable.

## Cross-depth observation

There's a deeper structural point hiding in the perspective
primitive. An observer of a locus is *itself* a locus
somewhere in the tower — possibly far above. The depth gap
between observer and observed determines what shape the
observation takes:

- **Small depth gap** — the observer is the locus's direct
  parent. Observation goes through the contract block, in
  the same process, with the parent reading exposed fields
  directly.
- **Medium gap** — the observer is several layers above,
  possibly across the bus. A `perspective` declaration is
  the right shape: typed, serializable, validated.
- **Large gap** — the observer is in a completely separate
  process or binary, possibly across the network. Same
  perspective primitive; the transport binding (Unix socket,
  TCP, NATS) carries it across.

What looks at a casual glance like "different mechanisms for
local-vs-remote observation" is one mechanism — the
perspective — applied at different depths in the locus
tower. The *content* changes (which fields are useful to
ship across a process boundary vs. within one) but the
*form* doesn't.

This is also why "cross-depth observation" reads as a
*projection axis* in Hale: the depth of the observer
relative to the observed determines the resolution at which
observation happens, just as projection class determines the
resolution at which a parent serves observations of its
children (see [Capacity & storage](./capacity-storage.md)).

## When you'll use this

In practice, a perspective is the right tool when:

- You have a *parameter-fitting* pipeline and a separate
  *parameter-applying* binary.
- You want to *hot-reload* configuration into a long-running
  service without restarting it, with strong type guarantees
  about the new config matching the schema.
- You need *cross-process state propagation* between
  cooperating binaries that share source.

For most application code — single-binary services, in-
process bus communication, local handlers — you won't reach
for `perspective`. Your locus's `params` and `bus`
subscriptions cover the surface. Perspective is the tool you
pull out when state needs to cross a process boundary with
schema discipline intact.

## Next

The final Concepts chapter,
[Modeling — how to think in Hale](./modeling.md), is the
synthesis: how to take everything from the previous chapters
and use it to model a real system, what the idiomatic
patterns look like, what to do when the language seems to
resist your design.
