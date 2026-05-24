<!-- Synced from hale-lang/pond/sessions/README.md by tools/sync-pond-docs.sh — do not edit here. -->

# pond/sessions — HMAC-signed cookie sessions

Suggested import alias: **`sess`**

```hale
import "vendor/pond/sessions" as sess;
```

Stateless, signed, cookie-based sessions. The server holds one
secret; every cookie is `session=<base64(payload)>.<base64(hmac)>`,
verified per-request via HMAC-SHA256. No server-side session table,
no Redis, no sticky load balancing — the cookie *is* the state.

Depends on [`pond/crypto`](./crypto.md) (`hmac_sha256` +
`constant_time_eq`).

## Surface

```hale
type Session       { id, data }                   // pure data
type SessionError  { kind }                       // fallible payload

locus SessionStore {
    params { secret: Bytes; ttl_seconds: Int = 86400; }
    fn read(cookie_header) -> Session     // sentinel + last_error on err
    fn write(s)            -> String      // Set-Cookie value
    fn invalidate(id)      -> String      // clearing Set-Cookie value
}

// Public free-fn surface. Cross-seed non-fallible calls lower
// directly post-A3 (hale f9068fa) — no namespace-lotus
// indirection required.
fn sign_payload(secret, session, now_seconds, ttl_seconds) -> String;
fn get_value(s, key)         -> String;                // "" if absent
fn set_value(s, key, val)    -> Session;               // pure
fn now_seconds()             -> Int;                   // monotonic clock
fn encode_cookie_value(payload, mac)   -> String;
fn extract_session_cookie(cookie_header) -> String;

// Fallible free fn — canonical verify surface; consumers
// address the failure with `or` directly.
fn verify_cookie(secret, cookie_header, now_seconds)
    -> Session fallible(SessionError);
```

`Session.data` is a tab-separated `k1=v1\tk2=v2\t...` packed
block. Use `get_value` / `set_value` rather than touching the
field directly — the codec is the same family as
`pond/router::RouteParams.path_kv` and `pond/metrics::Labels.kv`
(see FRICTION.md `duplicate-suspected`).

## Quick start with `std::http::Server`

```hale
import "vendor/pond/sessions" as sess;
import "vendor/pond/crypto"  as crypto;

locus Routes {
    params {
        // One SessionStore shared across every request. The
        // secret is loaded once at boot (in main).
        sessions: sess::SessionStore;
    }
    fn handle(req: std::http::Request) -> std::http::Response {
        let cookie = std::http::header(req, "Cookie");
        let s      = self.sessions.read(cookie);

        if self.sessions.last_error.kind == "" {
            // Authenticated.
            let who = sess::get_value(s, "user");
            return std::http::Response {
                status: 200,
                body:   "hello " + who
            };
        }

        // Issue a new session for the first-time visitor.
        let fresh = sess::set_value(
            sess::Session { id: "u-1", data: "" },
            "user", "alice"
        );
        let set_cookie = self.sessions.write(fresh);

        // `std::http::Response.headers` (C11, hale `965d828`)
        // ships the CRLF-joined header block — Set-Cookie attaches
        // there instead of the body. The wire shape is the canonical
        // one; nothing pond-specific in the response layout.
        return std::http::Response {
            status:  200,
            headers: "Set-Cookie: " + set_cookie,
            body:    "welcome"
        };
    }
}

fn main() {
    // Load the secret from an env var; production deploys would
    // route through a KMS instead.
    let secret_hex = std::env::var("SESSION_SECRET");
    let secret     = crypto::hex_decode(secret_hex) or raise;

    std::http::Server {
        port:    8080,
        handler: Routes {
            sessions: sess::SessionStore {
                secret:      secret,
                ttl_seconds: 3600
            }
        }
    };
}
```

## Wire format

```
session=<base64(payload)>.<base64(hmac)>
```

`payload` is UTF-8 bytes of `<id>\t<expires_at>\t<data>`. `hmac`
is HMAC-SHA256(secret, payload). Both halves are base64-encoded
with `std::text::base64::encode`. Verification recomputes the
HMAC, compares against the received mac in constant time
(`crypto::constant_time_eq`), then checks the expiry stamp.

## Error shape

`SessionError.kind` is one of:

- `"missing"`   — no `session=` cookie in the request's `Cookie:` header.
- `"malformed"` — present but not `<b64>.<b64>`, or empty after decode.
- `"tampered"` — HMAC mismatch (constant-time compare failed).
- `"expired"`  — payload's TTL deadline is in the past.

The free-fn `verify_cookie` returns these via `fallible(SessionError)`;
the `SessionStore.read` method surfaces the same kinds on
`self.last_error` (per the two-channel deviation below).

## Contract deviations

- `SessionStore.read` is declared without `fallible(SessionError)`.
  Per `spec/semantics.md § Fallible call semantics`, locus methods
  on user-declared loci may not declare `fallible(E)` — that
  channel is reserved for free fns. The companion free fn
  `verify_cookie(secret, header, now)` IS fallible and is the
  recommended surface for consumers that want `or` addressing.
  See `FRICTION.md` for the wider trend across `pond/CONTRACTS.md`.

- TTL uses `std::time::monotonic()` (process-local) because the
  v1 stdlib has no wall-clock `time::now()`. Cookies don't
  survive a process restart for expiry-comparison purposes —
  flagged in `FRICTION.md`.

## Building

```
$ hale build \
    pond/sessions/
```
