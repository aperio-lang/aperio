<!-- Synced from aperio-lang/pond/http/client/README.md by tools/sync-pond-docs.sh — do not edit here. -->

# pond/http/client

HTTP/1.1 client built on `std::io::tcp::*` plus
`std::io::tls::*`. Exposes `get` / `post` / `request` free fns
for one-shot calls, plus a `Client` locus with a connection-
pool slot set and retry-with-backoff for callers that want a
stable per-host handle. Returns `Response` or
`fallible(HttpError)` on the free-fn surface; the `Client`
methods route value-channel errors into `self.last_error_*()`
accessors per the two-channel rule.

Both `http://` and `https://` URLs work — the scheme picks
between plain TCP and TLS at connect time, and the
internal `__HttpConn` wrapper dispatches `send_bytes` /
`recv_bytes` to the right substrate. Hostnames resolve via
getaddrinfo (`std::io::tcp::connect`, upstream C6).

Suggested import alias: `http`.

```aperio
import "vendor/pond/http/client" as http;

let r = http::get("https://example.com/") or raise;
println("status=", r.status, " body=", std::str::from_bytes(r.body));
```

See [`examples/get-demo/`](https://github.com/aperio-lang/pond/tree/main/http/client/examples/get-demo/) for a runnable
end-to-end demo.
