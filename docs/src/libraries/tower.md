<!-- Synced from aperio-lang/pond/tower/README.md by tools/sync-pond-docs.sh — do not edit here. -->

# pond/tower — Multi-tower application substrate

Suggested import alias: **`tower`**

```aperio
import "vendor/pond/tower" as tower;
```

A multi-tower app is a binary running multiple independent locus
trees ("towers") under one process. Each tower handles its own
concern — HTTP serving, metrics endpoint, market-data ingest, job
pool, agent loop, etc. — and the app composes them in a single
`fn main()`.

`pond/tower` is the shared substrate that lets these towers
coexist coherently: a single `Tower` interface every tower-shape
locus targets, a registry that holds references to running
towers, and small free-fn conveniences for banner output and
name-based lookup.

## Design tenets

- **Caller-defined names.** A tower's identity is a
  `name: String` param on its locus, supplied at instantiation.
  Two `Router`s named `"public"` and `"admin"` are different
  towers.
- **No hidden behavior.** An empty `fn main()` does nothing.
  Towers register only when the app explicitly calls
  `registry.add(t)`. Logging, metrics, health endpoints, drain
  policies are all composed by the app — `pond/tower` installs
  nothing on its own.
- **Pull registration.** The registry holds references; towers
  are owned by whoever instantiated them (usually the app's
  implicit-locus). The registry is observational, not structural
  — towers are NOT F.11 children of the registry.
- **Builders are the deferred-init carrier.** Aperio v1 has no
  closures-with-capture; the `TowerBuilder` interface fills that
  role. A builder locus captures the would-be tower's config in
  its own params, and a `build() -> Tower` method instantiates
  on demand.

## Surface (v1)

```aperio
// interfaces
interface Tower         { fn name() -> String; }
interface TowerBuilder  { fn build() -> Tower; }

// shapes
type Status {
    state:         String = "running";   // running|draining|failed|dissolved
    uptime_s:      Int    = 0;
    last_error:    String = "";
    restart_count: Int    = 0;
}

// registry — @form(vec) of Tower, no user methods
@form(vec)
locus Registry { capacity { heap items of Tower; } }

// non-fallible free fns over Registry
fn add(r: Registry, t: Tower);                              // register a tower
fn count(r: Registry) -> Int;                               // number registered
fn name_at(r: Registry, i: Int) -> String;                  // name at index
fn tower_at(r: Registry, i: Int) -> Tower;                  // handle at index
fn has(r: Registry, name: String) -> Bool;                  // presence predicate
fn find_or(r: Registry, name: String, fallback: Tower) -> Tower;
fn startup_banner(r: Registry);
fn names(r: Registry) -> String;
```

**v1 is non-fallible by design.** Cross-seed mangling of struct
literals and bare-fn-call expressions under `fail` / `or fail`
keywords hits a codegen gap (see `FRICTION.md`); v1 dodges by
shipping only non-fallible operations. `find_or` takes an
explicit fallback Tower from the caller; `has` covers "branch
first, then look up." A fallible `lookup` shape will land when
the cross-seed mangler walks `or fail` expression positions.

## Tower-shape library convention

Any pond/std/user library meant to be a tower exports a primary
locus that:

1. Takes a required `name: String` param (no default — forces
   the caller to choose an identity).
2. Exposes `fn name() -> String { return self.name; }`.

That's the entire opt-in. Field `name` and method `name()`
coexist cleanly under F.17 (call-site shape disambiguates: bare
`r.name` is the field, `r.name()` is the method).

If the library also wants to offer deferred-init, it exports a
sibling builder locus satisfying `TowerBuilder` whose `build()`
returns the concrete tower (which structurally coerces to
`Tower` at the return site post-G20).

## Canonical multi-tower main

```aperio
import "vendor/pond/tower" as tower;
import "vendor/pond/router" as router;
import "vendor/pond/metrics" as metrics;

fn main() {
    // Register FIRST — dissolves last, after every tower it
    // holds a reference to.
    let reg = tower::Registry { };

    // Towers — let-bound at main's scope.
    let r = router::Router    { name: "public",  port: 8080 };
    let m = metrics::Registry { name: "metrics", port: 9100 };

    tower::add(reg, r);
    tower::add(reg, m);

    tower::startup_banner(reg);

    // ... long-running work ...
    // SIGINT → drain cascade in reverse instantiation order
    // (m, r, reg). The runtime handles it; pond/tower doesn't.
}
```

## Builder usage (deferred-init)

When a tower needs to be carried as data — e.g. a list of "towers
to start" handed to a coordinator that decides when to spawn each
one — use the builder pattern:

```aperio
locus RouterBuilder {
    params { name: String; port: Int; }
    fn build() -> Tower {
        return Router { name: self.name, port: self.port };
    }
}

fn main() {
    let b = RouterBuilder { name: "public", port: 8080 };
    // ... pass b around, store it in a vec, dispatch on tag ...
    let r = b.build();   // tower instantiated here, in main's frame
}
```

A tower instantiated via `build()` lives in the caller of
`build()`'s arena (m49 deep-copy on interface return). Plan its
lifetime around that frame, not the builder's.

## What pond/tower does NOT provide

By design, none of these are auto-installed. Compose them
yourself if your app wants them:

- **No observability rail.** No `std::log` sinks instantiated, no
  metrics endpoint mounted, no tracing span tree. Use
  `std::log::StdoutSink`, `pond/metrics`, `pond/tracing` directly.
- **No supervision policy.** No automatic restart, no Erlang
  intensity gate. Use `pond/supervisor` (single-accept-type
  workers today) or compose your own `on_failure` body.
- **No shutdown coordinator.** The runtime's SIGINT → drain
  cascade is already in place; pond/tower doesn't wrap or
  augment it.
- **No health endpoint.** No `/healthz` mount. Compose a
  `std::http::Handler` over the registry yourself.
- **No subcommand dispatch.** Use `std::env` to parse argv.
- **No config layering.** Use `std::cli::Resolver` directly.

These pieces may grow in pond eventually (under `pond/tower/*`
sub-libs or as separate pond libraries), but each ships only when
a real consumer demands it.

## Relation to existing pond libs

Existing pond libraries become tower-shape with a one-line
addition: a `name: String` param and a `fn name() -> String {
return self.name; }` method. The migration is opportunistic;
libraries that don't want to be towers (`pond/_util/*`,
`pond/crypto`'s free-fn surface, type-only libraries) don't add
it.

`pond/supervisor` currently accepts only its own `Worker` locus
(single-accept-type per F.11). A heterogeneous-tower supervisor —
accept any `Tower` interface value, dispatch restart policies per
tower name — is a natural follow-up but not in v1's pond/tower
scope.

## Building the demo

```bash
aperio build pond/tower/examples/multi-tower-demo/
./pond/tower/examples/multi-tower-demo/multi-tower-demo
```

## Files

- `interfaces.ap` — `Tower` and `TowerBuilder` interfaces
- `types.ap` — `Status` and `RegistryError` shapes
- `registry.ap` — `Registry` locus + `has` / `lookup` / `at` free fns
- `banner.ap` — `startup_banner` / `names` free fns
- `examples/multi-tower-demo/` — three-tower demo
