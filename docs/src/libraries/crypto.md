<!-- Synced from aperio-lang/pond/crypto/README.md by tools/sync-pond-docs.sh — do not edit here. -->

# pond/crypto

Suggested alias: `crypto`.

```aperio
import "vendor/pond/crypto" as crypto;
```

Minimal cryptographic primitives for Aperio apps: SHA-256,
HMAC-SHA256, hex encode/decode, constant-time compare, and a
`random_bytes` shim. SHA-256 and HMAC-SHA256 delegate to
`std::crypto::sha256` / `std::crypto::hmac_sha256` (the C-backed
primitives landed in lotus-lang `83f87ff`). The
`examples/sign-demo/` still exercises the FIPS 180-2 vectors for
`"abc"`, the empty string, and the 56-byte B.2 input, plus the
RFC 4231 test-case-1 HMAC-SHA256 vector — they now verify the
C-backed implementation against the published vectors.

`random_bytes` delegates to `std::os::getrandom` (the CSPRNG that
landed in lotus-lang `ef85ed5` — `getrandom(2)` syscall with a
`/dev/urandom` fallback), so the surface is safe for key material
and session secrets. The public free fn is non-fallible — IoError
substitutes empty Bytes; callers wanting the typed error can call
`std::os::getrandom(n) or <handler>` directly.

The public API is bare free fns, matching CONTRACTS.md (the
namespace-lotus workaround for the cross-seed non-fallible call
gap fell away with lotus-lang `f9068fa` (A3)). Canonical use
shape (see `examples/sign-demo/main.ap`):

```aperio
let digest = crypto::sha256(payload);
let tag    = crypto::hmac_sha256(key, message);
let hex    = crypto::hex_encode(tag);
let bs     = crypto::hex_decode(hex) or raise;   // fallible free fn
if crypto::constant_time_eq(tag, recomputed) { ... }
```
