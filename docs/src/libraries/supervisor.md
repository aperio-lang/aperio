<!-- Synced from hale-lang/pond/supervisor/README.md by tools/sync-pond-docs.sh — do not edit here. -->

# pond/supervisor — Erlang OTP-style supervision trees

Suggested import alias: **`sup`**

```hale
import "vendor/pond/supervisor" as sup;
```

Erlang OTP's supervision-tree pattern grafted onto Hale's built-in
`on_failure` handler + the `restart` / `restart_in_place` / `bubble`
recovery primitives (per `spec/runtime.md` § Recovery primitives and
F.9 Collapse vs explosion in `spec/design-rationale.md`).

## Surface

```hale
type SupStrategy { kind: String; }
//   kind ∈ { "one_for_one", "rest_for_one", "one_for_all", "escalate" }

type ChildSpec { name: String; policy: String; }
//   policy ∈ { "permanent", "transient", "temporary" }
//   (CONTRACTS.md spells this field "restart"; renamed to "policy"
//   because "restart" is a reserved keyword — see FRICTION.md.)

locus Worker {
    params {
        name:          String = "worker";
        kind:          String = "generic";
        payload:       String = "";
        fail_on_birth: Bool   = false;
        birth_order:   Int    = 0;
        last_error:    String = "";
    }
    closure failed { captures: name, last_error; epoch inline; }
    fn fail(reason: String);                 // structural-failure trigger
}

locus Supervisor {
    params {
        strategy:       SupStrategy;
        max_restarts:   Int = 3;
        window_seconds: Int = 60;
        // internal counters — restart_count, window_start_ns,
        // children_seen, spec_count, spec_names, spec_restarts
    }
    fn add_child(spec: ChildSpec, child_name: String);
    fn spawn_worker(name: String, kind: String, payload: String,
                    fail_on_birth: Bool);
    // accept(c: Worker) + on_failure(c, err) wire up automatically.
}
```

## Strategies → Hale recovery primitives

The whole library is one fact: which recovery primitive does each
strategy fire from `on_failure`? Mapping (per
`spec/runtime.md` § Recovery primitives):

| Strategy        | Body of `on_failure`                                            | v1 status |
|-----------------|-----------------------------------------------------------------|-----------|
| `one_for_one`   | `restart(c);` — re-run just the failed child's birth.           | shipped, exercised by `examples/one-for-one/`. |
| `rest_for_one`  | restart `c` AND every sibling later in birth order.             | degraded — needs sibling iter; see FRICTION.md. |
| `one_for_all`   | restart `c` AND every other child.                              | degraded — needs `for sib in self.children { restart(sib); }`; see FRICTION.md. |
| `escalate`      | `bubble(err);` — propagate to the supervisor's parent.          | shipped. |

`restart_in_place(c)` (factory-reset variant; see
`spec/runtime.md` § Recovery primitives) is available to consumers
who want it but the supervisor's built-in branches use plain
`restart(c)`. A future hook (`spec/...` v1.x) could let `ChildSpec`
select `restart` vs `restart_in_place`; deferred until a real
workload distinguishes the two.

## ChildSpec.policy

The per-child filter that runs BEFORE the strategy:

- `"permanent"` — restart per strategy on every failure (default).
- `"transient"` — restart only on abnormal exit
  (ClosureViolation). Clean dissolves are absorbed.
  In Hale v1, `on_failure` ONLY runs on ClosureViolation (a clean
  collapse never invokes the handler per F.11), so this currently
  has the same effect as `"permanent"`. The branch is documented
  and ready for the day Hale adds a clean-exit failure type.
- `"temporary"` — never restart; absorb every failure.

## Restart intensity gate

`max_restarts` failures within `window_seconds` triggers
`bubble(err)` regardless of strategy — the supervisor itself
escalates rather than continue restarting. Defaults (3 in 60s)
match Erlang OTP's traditional `MaxR` / `MaxT`. The gate uses
`std::time::monotonic()` (process-local; not wall-clock).

## Worker — the supervised child

`Supervisor.accept(c: Worker)` only admits the lib's `Worker` locus
type (v1's F.11 single-accept-type rule). Workers carry:

- `name` — matched against the spec table for policy lookup.
- `kind` / `payload` — application-defined strings; the user
  branches on `kind` inside `run()` for per-child behavior.
- `fail_on_birth` — deterministic-fail flag for tests/demos.
- `last_error` — readable through the child handle in the
  parent's `on_failure(c, err)`.
- `fail(reason)` — error-check-fn-style escalation (`spec/styleguide.md`
  § 7); sets `last_error` and `violate failed;`.

## Use shape

```hale
import "vendor/pond/supervisor" as sup;

locus App {
    run() {
        let s = sup::Supervisor {
            strategy: sup::SupStrategy { kind: "one_for_one" },
        };
        s.add_child(sup::ChildSpec { name: "ingest", policy: "permanent" }, "ingest");
        s.add_child(sup::ChildSpec { name: "format", policy: "transient" }, "format");
        s.add_child(sup::ChildSpec { name: "logger", policy: "temporary" }, "logger");

        s.spawn_worker("ingest", "kafka", "topic=events", false);
        s.spawn_worker("format", "json",  "schema=v1",     false);
        s.spawn_worker("logger", "stdout","",              false);
        // ... workers run; if any violates the supervisor's
        // on_failure picks one_for_one + the per-child policy.
    }
}

fn main() {
    App { };
}
```

## Contract deviations

Three deviations from `pond/CONTRACTS.md § pond/supervisor/`:

1. `ChildSpec.restart` → `ChildSpec.policy`. `restart` is a reserved
   keyword (recovery primitive name); the field is parser-rejected.
2. `Supervisor.add_child(spec, child: LocusRef)` →
   `Supervisor.add_child(spec, child_name: String)`. `LocusRef` is
   not a user-typeable parameter in v1 (it's the internal C-ABI
   shape for `self.children`/recovery-op args). Children join via
   the `accept(c: Worker)` path; `add_child` records only the spec.
3. `Supervisor.spawn_worker(...)` is an additional method, not in
   the contract. It exists because children must be instantiated
   INSIDE the supervisor's own method body for `accept` to fire on
   the supervisor (per spec/semantics.md § Locus instantiation).

See `FRICTION.md` for the wider list (rest_for_one / one_for_all
degradations, codegen-rejects-violate-in-birth, Duration → Int
conversion shim, etc.).

## Building

```
$ hale check \
    pond/supervisor/
ok: 3 file(s) typechecked
$ hale build \
    pond/supervisor/examples/one-for-one/
built: .../examples/one-for-one/one-for-one
```

`hale build` on the lib directory alone fails with "program has
no `fn main()`" — the standard pond-lib status (subprocess, math/
stats and all of pond hit the same). The lib's source typechecks
clean; the example binary builds and runs.
