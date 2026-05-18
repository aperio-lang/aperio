<!-- Synced from aperio-lang/pond/http/client/README.md by tools/sync-pond-docs.sh — do not edit here. -->

# pond/http/client

HTTP/1.1 client built on `std::io::tcp::Stream`. Exposes
`get` / `post` / `request` free fns for one-shot calls, plus a
`Client` locus with a connection-pool slot set and retry-with-
backoff for callers that want a stable per-host handle. Returns
`Response` or `fallible(HttpError)` on the free-fn surface;
the `Client` methods route value-channel errors into
`self.last_error_*()` accessors per the two-channel rule.

HTTPS is out of scope at v1 (no TLS in stdlib). DNS resolution
is out of scope at v1 — the underlying `lotus_tcp_connect` only
accepts IPv4 dotted-quad hosts. Both gaps are logged in
[`FRICTION.md`](https://github.com/aperio-lang/pond/blob/main/http/client/FRICTION.md).

Suggested import alias: `http`.

```aperio
import "vendor/pond/http/client" as http;

let r = http::get("http://127.0.0.1:8080/health") or raise;
println("status=", r.status, " body=", std::str::from_bytes(r.body));
```

See [`examples/get-demo/`](https://github.com/aperio-lang/pond/tree/main/http/client/examples/get-demo/) for a runnable
end-to-end demo.
