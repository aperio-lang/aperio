<!-- Synced from aperio-lang/pond/router/README.md by tools/sync-pond-docs.sh — do not edit here. -->

# pond/router

HTTP router on top of `std::http`. Routes have a method + a
pattern (with `:name` captures); middleware is a chain of
fn-pointer pairs that run `before` (transform Context) and
`after` (transform Response). The Router locus implements
`std::http::Handler` structurally, so it drops straight into a
`std::http::Server { handler: my_router, ... }`.

Suggested alias: `router`.

## Vendoring

```aperio
import "vendor/pond/router" as router;
```

## Quick start

```aperio
import "vendor/pond/router" as router;

fn root(ctx: router::Context) -> router::Response {
    return router::Response { status: 200, body: "hello" };
}

fn greet(ctx: router::Context) -> router::Response {
    let name = router::path_param(ctx.params, "name");
    return router::Response {
        status: 200,
        body: "hello, " + name
    };
}

fn log_before(ctx: router::Context) -> router::Context {
    eprintln(ctx.method, " ", ctx.path);
    return ctx;
}

fn main() {
    let r = router::Router { };
    r.add("GET", "/", root);
    r.add("GET", "/greet/:name", greet);
    r.use_before(log_before);
    std::http::Server {
        port: 8080,
        handler: r,
        ready_signal: "READY"
    };
}
```

`use_before` / `use_after` are convenience methods for the
common one-sided shapes; `use_mw(before, after)` takes both
halves when a middleware needs to act on both directions. The
two-fn-pointer shape is forced by the v1 storage constraint
documented in `FRICTION.md` — interface values can't yet sit
in `@form(vec)` cells, so middleware is registered as fn
pointer pairs rather than a single `Middleware`-typed value.

## Public surface

Implements the `pond/router/` section of
[`../CONTRACTS.md`](https://github.com/aperio-lang/pond/blob/main/CONTRACTS.md), with two storage-driven
deviations:

- `Router.add(method, pattern, h)` takes
  `h: fn(Context) -> Response`, not `h: Handler`.
- `Router.use(m)` is named `use_mw(before, after)` (the
  `use` token is reserved, and the v1 storage constraint
  splits the `Middleware` interface into its two fn halves).

Both deviations preserve the call-site shape: a consumer still
declares a free fn (or namespace-lotus method, via a thin
adapter fn) whose signature matches and passes it by name. See
[`FRICTION.md`](https://github.com/aperio-lang/pond/blob/main/router/FRICTION.md) for the why and the path to
restoring the literal contract once the F.20 Phase B
follow-up lands.

The `Handler` and `Middleware` interfaces themselves are still
declared in `interfaces.ap` so consumers can name them in
their own free-fn / locus-method signatures for forward
compatibility.

## Demo

`examples/hello-routes/` ships a runnable demo: `GET /`
returns "hello", `GET /greet/:name` returns "hello, NAME", and
a logging middleware writes each request line to stderr. Build
+ run:

```bash
aperio build pond/router/examples/hello-routes/
./pond/router/examples/hello-routes/hello-routes
# in another shell:
curl -s http://127.0.0.1:8080/
curl -s http://127.0.0.1:8080/greet/world
```

The demo prints `READY` on stderr when the listen socket binds
(via `std::http::Server.ready_signal`); test oracles wait for
that line before issuing requests.

## Files

| File | What |
|------|------|
| `types.ap` | `RouteParams`, `Context`, `Response`, `Route`, `MwEntry` shapes |
| `interfaces.ap` | `Handler`, `Middleware` structural interfaces |
| `lists.ap` | `@form(vec)` route + middleware lists |
| `match.ap` | Pattern split + match + path/query extraction |
| `params.ap` | `path_param` / `query_param` free fns |
| `router.ap` | `Router` locus + default 404 + dispatch chain |
