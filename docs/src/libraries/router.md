<!-- Synced from hale-lang/pond/router/README.md by tools/sync-pond-docs.sh — do not edit here. -->

# pond/router

HTTP router on top of `std::http`. Routes have a method + a
pattern (with `:name` captures); middleware is a chain of
`Middleware`-shaped loci that run `before` (transform Context)
and `after` (transform Response). The Router locus implements
`std::http::Handler` structurally, so it drops straight into a
`std::http::Server { handler: my_router, ... }`.

Suggested alias: `router`.

## Vendoring

```hale
import "vendor/pond/router" as router;
```

## Quick start

```hale
import "vendor/pond/router" as router;

locus Root {
    fn handle(ctx: router::Context) -> router::Response {
        return router::Response { status: 200, body: "hello" };
    }
}

locus Greet {
    fn handle(ctx: router::Context) -> router::Response {
        let name = router::path_param(ctx.params, "name");
        return router::Response {
            status: 200,
            body: "hello, " + name
        };
    }
}

locus LogMw {
    fn before(ctx: router::Context) -> router::Context {
        eprintln(ctx.method, " ", ctx.path);
        return ctx;
    }
    fn after(ctx: router::Context, resp: router::Response) -> router::Response {
        return resp;
    }
}

fn main() {
    let r = router::Router { };
    router::add(r, "GET", "/", Root { });
    router::add(r, "GET", "/greet/:name", Greet { });
    router::use_before(r, LogMw { });
    std::http::Server {
        port: 8080,
        handler: r,
        ready_signal: "READY"
    };
}
```

Handlers and middleware are Hale loci that structurally
satisfy `router::Handler` (one `handle` method) and
`router::Middleware` (a `before` + `after` pair) respectively.
A "before-only" middleware writes the interesting logic in
`before` and a passthrough `after` — same for "after-only".

`use_before` / `use_after` / `use_mw` are all aliases at v1 —
they each push a single `Middleware` value onto the chain.
The three names exist because they read naturally at the call
site and pre-G20 they were distinct shapes (the
`use_before(fn)` / `use_after(fn)` fn-pointer convenience halves
collapsed into Middleware loci when interface storage landed).

## Public surface

Implements the `pond/router/` section of
[`../CONTRACTS.md`](https://github.com/hale-lang/pond/blob/main/CONTRACTS.md), with two storage-driven
deviations remaining:

- The registry-shape ops (`add`, `use_mw`, `use_before`,
  `use_after`) are free fns over `Router` rather than `Router`
  methods. v1 codegen doesn't apply the `LocusRef → Interface`
  coercion at user-declared locus-method arg sites; free-fn
  arg sites DO coerce. Call shape:
  `router::add(r, "GET", "/", Root { })`.
- `Router.use(m)` is named `Router.use_mw(m)` (free fn:
  `router::use_mw(r, m)`). `use` is on the reserved-keyword
  shortlist and we side-step it.

Both deviations preserve the call-site shape: a consumer writes
a handler / middleware locus and passes it by name. See
[`FRICTION.md`](https://github.com/hale-lang/pond/blob/main/router/FRICTION.md) for the why and the path to
restoring the literal contract once cross-seed method-arg
interface coercion lands upstream.

## Demo

`examples/hello-routes/` ships a runnable demo: `GET /`
returns "hello", `GET /greet/:name` returns "hello, NAME", and
a logging middleware writes each request line to stderr. Build
+ run:

```bash
hale build pond/router/examples/hello-routes/
./pond/router/examples/hello-routes/main
# in another shell:
curl -s http://127.0.0.1:8080/
curl -s http://127.0.0.1:8080/greet/world
```

The demo prints `READY` on stdout when the listen socket binds
(via `std::http::Server.ready_signal`); test oracles wait for
that line before issuing requests.

## Files

| File | What |
|------|------|
| `types.hl` | `RouteParams`, `Context`, `Response` shapes |
| `interfaces.hl` | `Handler`, `Middleware` structural interfaces |
| `lists.hl` | `@form(vec)` storage loci (methods, patterns, handlers, middleware) |
| `match.hl` | Pattern split + match + path/query extraction |
| `params.hl` | `path_param` / `query_param` free fns |
| `router.hl` | `Router` locus + `NotFound404` default + dispatch chain + free-fn register API |
