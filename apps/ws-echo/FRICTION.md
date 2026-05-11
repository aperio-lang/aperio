# ws-echo friction log

> Per-app friction encountered while building `apps/ws-echo/`.
> The global friction log at `notes/aperio-friction.md` carries
> the same entries with the same dates (the compiler session
> reads the global file). This per-app copy stays put so a
> reader of the app can see what was open without cross-
> referencing.

## Format

Same as the global log. Append-only.

## Entries

<!-- New entries below. -->

## 2026-05-11 bytes-construction-from-ints

**Tried:** Build the outbound WebSocket frame header as a
Bytes value (first byte is 0x81 for FIN+text, etc.; every
FIN-set frame has byte ≥ 0x80).
**Hit:** No constructive surface for Bytes — `std::bytes::
from_string` strlen-measures, Aperio source is ASCII-only,
and there's no `bytes::from_int`, `bytes::concat`, or
`bytes::from_ints`.
**Workaround:** Send-side methods (`WsConnL.send_text`,
`.send_close`, `.send_pong`) STUBBED with diagnostics
pointing here. Parse-side fully works.
**Why it matters:** Library is currently read-only. Every
binary-protocol writer (HTTP/2, RPC, encoded images) hits the
same wall. Minimal unblock: `std::bytes::from_int(b: Int) ->
Bytes` + `std::bytes::concat(a, b) -> Bytes`.

See `notes/aperio-friction.md` for the full entry.

## 2026-05-11 bitwise-int-binops-not-lowered

**Tried:** Natural form for frame-bit extraction
(`b & 0x80`, `b & 0x0F`, `b & 0x7F`) and per-byte unmask
(`b ^ mask_byte`).
**Hit:** Parser + AST accept `&`, `|`, `^`, `<<`, `>>`;
codegen errors with `binop {BitAnd,BitOr,BitXor,Shl,Shr} on
Int`.
**Workaround:** Arithmetic emulation — `b >= 128` for high
bit, `b % 16` for low nibble, `b % 128` for low 7 bits;
per-bit XOR walk in `frame.ap` `ws_xor8`. Linear in 8;
correct but ~8× the cost of a native xor.
**Why it matters:** Every protocol-bit shape uses these.
LLVM has `xor`, `and`, `or`, `shl`, `lshr` as direct IR; the
existing `lower_binop_int` switch needs five more arms.

## 2026-05-11 sha1-base64-missing

**Tried:** Compute `Sec-WebSocket-Accept` = `base64(SHA1(key
+ magic-uuid))` for the upgrade response (RFC 6455 §4.2.2).
**Hit:** No `std::hash` / `std::crypto` / `std::encoding`
namespaces in stdlib; no `lotus_sha1` / `lotus_base64_*` in
C runtime.
**Workaround:** `WsConnL.do_handshake()` STUBBED — reads
upgrade bytes, prints diagnostic, returns false.
**Why it matters:** Gates browser-compatible WebSocket
entirely. Also gates HTTP digest auth, JWT, content-
addressed storage, file checksums, basic auth headers,
data URIs.

## 2026-05-11 http-request-headers-absent

**Tried:** Read `Sec-WebSocket-Key`, `Connection`,
`Upgrade`, `Sec-WebSocket-Version` from the incoming HTTP
upgrade request via `std::http::parse_request`.
**Hit:** `parse_request` returns `{method, path, version,
body}` — no headers field. The implementation explicitly
skips header lines (per the comment at `http.ap:26-31`).
**Workaround:** None for the handshake path. Would need to
re-implement header parsing in user code.
**Why it matters:** Every HTTP upgrade (WebSocket, HTTP/2),
every auth header, every content negotiation header. Min
unblock: `Request.header(name: String) -> String`.

## 2026-05-11 random-seed-missing

**Tried:** Generate a 32-bit random mask key per outbound
frame for future client-side build (RFC 6455 §5.3 mandates
fresh per-frame masks).
**Hit:** No `std::rand` / `std::random`, no `lotus_random`
in C runtime.
**Workaround:** Not blocking v0 (we built server-side only;
server frames are unmasked). Lowest-priority of the four.
**Why it matters:** Gates client-side WebSocket plus
anything else needing nonces, session IDs, jitter, fuzz
inputs. Min unblock: `std::rand::u32()` reading
`getrandom(2)` / `arc4random_buf`.
