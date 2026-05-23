<!-- Synced from aperio-lang/pond/jobs/README.md by tools/sync-pond-docs.sh — do not edit here. -->

# pond/jobs — background job queue + worker pool (BLOCKED on pond/sqlite)

SQLite-backed job queue (`Queue`) plus a pinned-worker pool
(`Pool` + `Worker`). At-least-once semantics with retry: a worker
that picks up a job either acks it on `JobResult.ok = true` or
fails it (with retry, until `max_attempts` is reached, then
dead-letter).

## Status (2026-05-16): BLOCKED

Transitively BLOCKED on the stdlib sqlite primitive: pond/jobs
imports pond/sqlite, and pond/sqlite is itself stub-mode until
`std::db::sqlite::*` ships in `runtime/stdlib/`. See
[`FRICTION.md`](https://github.com/aperio-lang/pond/blob/main/jobs/FRICTION.md) for the full chain.

The library type-checks and builds end-to-end against the
stubbed pond/sqlite. The Pool / Worker / handler shape is
exercised by the demo via the explicit `simulate: true` switch
(see Pool below); once pond/sqlite unblocks the same source
recompiles against a real SQLite-backed queue with no contract
change.

## Suggested alias

```aperio
import "vendor/pond/jobs" as jobs;
```

The bare `jobs` alias matches `pond/CONTRACTS.md`'s suggestion
and the `pond/README.md` catalog entry.

## Surface (as built)

```aperio
type Job       { kind: String; payload: String;
                 attempt: Int; max_attempts: Int; }
type JobResult { ok: Bool; detail: String; }
type JobError  { kind: String; detail: String; }

interface JobHandler {
    fn invoke(j: Job) -> JobResult;
}

locus Queue {
    params { db_path: String = ":memory:";
             table:   String = "pond_jobs";
             conn_handle: Int = -1;
             last_error:  String = ""; }
    birth()   { /* CREATE TABLE IF NOT EXISTS pond_jobs ... */ }
    dissolve(){ /* no-op */ }
}

fn enqueue(q: Queue, kind: String, payload: String,
           max_attempts: Int) -> Int  fallible(JobError);
fn dequeue(q: Queue)                  -> Job  fallible(JobError);
fn ack(q: Queue, job_id: Int)         -> Bool fallible(JobError);
fn fail_job(q: Queue, job_id: Int, retry: Bool) -> Bool fallible(JobError);

locus Pool {
    params { db_path: String = ":memory:";
             table:   String = "pond_jobs";
             workers: Int = 4;
             max_jobs_per_worker: Int = 0;
             simulate: Bool = false;
             handler: fn(Job) -> JobResult = default_handler; }
    accept(w: Worker) { }
    birth()      { /* spawn N pinned Workers as Pool children */ }
    run()        { /* no-op; workers carry the work */ }
    drain()      { /* cascade drains children first */ }
    dissolve()   { /* cascade pthread_joins each Worker */ }
    on_failure(w: Worker, err: ClosureViolation) {
        restart(w);
    }
}

locus Worker {
    params { worker_id: Int; db_path; table;
             max_jobs: Int; simulate: Bool;
             handler: fn(Job) -> JobResult; }
    run() { /* dequeue → invoke → ack/fail loop */ }
}
// Workers are placed pinned at deployment time via the
// consuming app's main locus `placement { }` block (F.31),
// e.g. `placement { worker_0: pinned(core = 1); }`.
```

### Deviations from CONTRACTS.md (all logged in FRICTION.md)

CONTRACTS.md sketches:

```aperio
locus Queue {
    params { db: Db; table: String = "pond_jobs"; }
    fn enqueue(...) -> Int fallible(JobError);
    fn dequeue() -> Job   fallible(JobError);
    fn ack(id)    -> ()   fallible(JobError);
    fn fail(id,r) -> ()   fallible(JobError);
}
locus Pool {
    params { queue: Queue; workers: Int = 4; handler: JobHandler; }
    birth() { /* spawn workers */ }
    drain() { /* finish in-flight */ }
}
```

Five v1 language gaps force the as-built shape:

1. **Locus methods can't be `fallible(E)`** (two-channel rule,
   `spec/semantics.md` § "Where each channel lives"). The four
   queue methods migrate to **free fns** taking a `Queue` ref
   in `query.ap`. Same translation pond/sqlite did for its
   `exec` / `query_one` / `prepare` etc. surface.

2. **Locus refs can't sit in another locus's params/fields**
   (`spec/types.md` § F.20 Phase B notes the gap for interfaces;
   the slot-restriction logic in `spec/semantics.md` extends to
   ordinary params — a stored Db / Queue would orphan its
   lifecycle when the holder dissolves). So:
   - `Queue.params.db: Db` → `Queue.params.db_path: String`
     (mirrors `pond/moa/`'s `SqliteStore`).
   - `Pool.params.queue: Queue` → `Pool.params.db_path` +
     `Pool.params.table` (the queue identity, not the queue
     handle).

3. **Interface values can't sit in locus params/fields**
   (`spec/types.md` § F.20 Phase B). `Pool.params.handler:
   JobHandler` → `Pool.params.handler: fn(Job) -> JobResult`.
   Same fn-pointer-shadow pattern pond/router/ uses for
   `Route.handler` / `MwEntry.before`. The
   `interface JobHandler` declaration in `interfaces.ap` stays
   as forward-compat scaffolding — once interface values can sit
   in fields the call site doesn't change.

4. **`-> () fallible(E)` return type rejected by codegen v0**
   (FRICTION.md item 12). `ack(id) -> () fallible(JobError)`
   and `fail(id, retry) -> () fallible(JobError)` both grow a
   `Bool` return (always `true` on success; divergent on
   failure). Same workaround pond/moa's `sqlite_put` uses.

5. **`fn fail()` shadows the `fail` keyword** inside its own
   body (FRICTION.md item 13). The dead-letter/retry surface is
   `fail_job(id, retry)` instead of `fail(id, retry)`.

Two stub-mode additions on `Pool`:

- `max_jobs_per_worker: Int = 0` — per-worker invocation cap.
  Necessary because workers are pinned (no inter-thread shutdown
  signal yet apart from mailbox-shutdown, and Worker doesn't
  declare `bus`) so the bounded-run shape is how `drain()`
  cascades cleanly.
- `simulate: Bool = false` — stub-mode switch. When `true`,
  workers bypass `dequeue` and synthesize Jobs so the handler
  fires `max_jobs_per_worker` times. The demo uses this; production
  code with pond/sqlite unblocked sets `simulate: false` (the
  default).

## Files

- `types.ap` — `Job`, `JobResult`, `JobError` (pattern 5).
- `interfaces.ap` — `JobHandler` interface decl (pattern 6
  scaffolding for the Phase B Pool.handler param).
- `queue.ap` — the `Queue` service locus (pattern 3, stub
  birth that runs CREATE TABLE through pond/sqlite).
- `query.ap` — the four queue free fns (pattern 6).
- `pool.ap` — `Pool` (cooperative service locus, pattern 3) +
  `Worker` (pinned service locus, also pattern 3).
- `examples/email-worker/main.ap` — enqueue 10 + 2-worker Pool
  demo. Runs in stub-mode (`simulate: true`); prints
  `sent: send_email:to=userN@example.com` per job.

## Example — enqueue + Pool

```aperio
import "vendor/pond/jobs" as jobs;

// Top-level handler — fn-pointer fields take a fn name, not a
// `self.method` reference (spec/types.md § "Types may hold
// fn(...) fields; dispatch via record.field(args)").
fn send_email(j: jobs::Job) -> jobs::JobResult {
    println("sending: ", j.payload);
    return jobs::JobResult { ok: true, detail: "delivered" };
}

locus App {
    run() {
        // Bring up the queue. Queue.birth() runs the schema
        // migration; let-binding means dissolve happens at
        // run()'s scope exit.
        let q = jobs::Queue { db_path: "/tmp/jobs.db" };

        // Enqueue a few jobs.
        let mut i = 0;
        while i < 10 {
            let _ = jobs::enqueue(q, "send_email",
                "to=user" + to_string(i) + "@example.com", 3)
                or 0;
            i = i + 1;
        }

        // Start a pool. 4 pinned worker pthreads spin up; each
        // exits after `max_jobs_per_worker` invocations. Let-
        // binding means Pool's dissolve (which pthread_joins
        // each worker) fires at scope-exit.
        let pool = jobs::Pool {
            db_path:             "/tmp/jobs.db",
            workers:             4,
            max_jobs_per_worker: 3,         // 4*3 = 12 jobs handled
            handler:             send_email,
        };
        let _ = pool;
    }
}

fn main() {
    App { };
}
```

The shape above is the post-unblock surface — exactly the code
you write once pond/sqlite is wired. The demo at
`examples/email-worker/main.ap` adds `simulate: true` so it runs
today against the stubbed backing.

## Verification

```bash
# Library typechecks (no `fn main()` — `build` would error
# "program has no `fn main()`", same as every other pond lib).
aperio check \
    pond/jobs/

# Example builds + runs end-to-end via the stub-mode workers
# (Pool { simulate: true }).
aperio build \
    pond/jobs/examples/email-worker/
pond/jobs/examples/email-worker/email-worker
```

Expected output:

```
[email-worker] enqueued 10 send_email jobs
sent: send_email:to=user0:0
sent: send_email:to=user0:1
sent: send_email:to=user0:2
sent: send_email:to=user0:3
sent: send_email:to=user0:4
sent: send_email:to=user1:0
sent: send_email:to=user1:1
sent: send_email:to=user1:2
sent: send_email:to=user1:3
sent: send_email:to=user1:4
[email-worker] pool drained; all workers joined
```

10 `sent:` lines, 2 workers (each handles 5), deterministic
completion. The `to=user<wid>:<n>` payload is what the simulate
mode synthesizes — when pond/sqlite unblocks and `simulate: true`
goes away, the same handler sees the real `to=userN@example.com`
payloads that the `jobs::enqueue` loop above puts into the
queue.

## When this unblocks

Once `std::db::sqlite::*` ships and pond/sqlite drops its
"unsupported" stubs:

1. `queue.ap` Queue.birth() runs the real CREATE TABLE; the
   schema lands in the SQLite file.
2. `query.ap`'s four bodies issue real `db::exec` / `db::query_one`
   calls. Today's `__bridge_db_*` paths stop firing on the happy
   path.
3. `pool.ap` Worker.run()'s `__run_real` branch starts processing
   real rows. The `simulate: true` switch goes away from the
   demo.
4. CONTRACTS.md is amended per FRICTION.md to reflect the
   two-channel + Phase-B-interface translations as the
   permanent shape (the gaps in 1–3 above will outlast the
   sqlite primitive itself; the contract amendments are the
   right long-term resolution).
