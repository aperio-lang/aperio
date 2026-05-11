# ws-echo

A WebSocket library as a lotus tower. Parser side works
end-to-end on top of Phase 2g's binary-safe TCP recv; sender
side and HTTP upgrade handshake wait on four more substrate
primitives (logged in `FRICTION.md`).

## Framing

The user's framing:

> i'm thinking like - it's lotus all the way down/up. we have
> tcp lotus -> websocket lotus -> application layer websocket
> lotus

That's the three tiers, mapped to source:

```
std::io::tcp::Stream      (substrate, shipped)
        ↓
WsConnL                   (this app — per-connection framing)
        ↓
WsServerL                 (this app — application-layer wrapper)
```

`WsServerL` wraps `std::io::tcp::Listener` with a per-
connection callback that instantiates a `WsConnL`. Each
`WsConnL` owns one TCP `Stream`, reads bytes via
`recv_bytes`, parses RFC 6455 frames out of the buffer,
returns typed `WsFrame` records.

## Lotus inventory

- **`WsConnL`** (`conn.ap`) — per-TCP-connection framing.
  Reads frames via `Stream.recv_bytes` + parser; send/handshake
  methods stubbed (see FRICTION).
- **`WsServerL`** (`server.ap`) — application-layer wrapper.
  Wires `std::io::tcp::Listener` with our `__ws_on_tcp_
  connection` callback.
- **`WsFrame`** (`frame.ap`) — parsed view of one frame.
  `{fin, opcode, masked, mask_key, payload_len, payload,
  valid}`. payload stays masked at v0 (constructive bytes
  needed for unmasked output — see FRICTION
  `bytes-construction-from-ints`); callers read unmasked bytes
  via `ws_unmasked_at(frame, i)`.
- **`WsMessage`** (`frame.ap`) — reassembled multi-frame
  message. v0 holds first-fragment only.
- **`__ws_parse_frame_bytes`** (`conn.ap`) — the parser body.
  Shared between stream-recv and file-fixture paths.
- **`ws_xor8`** (`frame.ap`) — per-bit XOR emulation (Aperio
  v0 codegen doesn't lower `^` on Int yet — see FRICTION
  `bitwise-int-binops-not-lowered`).

## How to run

```
cargo build --release -p aperio-cli
target/release/aperio build apps/ws-echo/
apps/ws-echo/ws-echo parse-fixture apps/ws-echo/fixtures/text-hello-masked.frame
```

The parse-fixture mode reads the 11-byte fixture file (a
real RFC 6455 frame: `0x81 0x85 0x37fa213d` mask + masked
"hello"), parses it, XOR-unmasks byte-by-byte, prints the
reconstructed text. Exit 0 silent after `all parse-fixture
assertions passed`. Assertion failures print
`ASSERTION FAILED: <label>` + exit 1.

Serve mode (`apps/ws-echo/ws-echo serve [host port]`)
brings up a `WsServerL` on 127.0.0.1:8080 with
`max_accepts: 1`. Accepts one TCP connection, reaches the
handshake stub, prints a diagnostic pointing at FRICTION,
exits. Verifies the Listener wiring compiles and the
architecture reaches the stubs end-to-end:

```
apps/ws-echo/ws-echo serve 127.0.0.1 18080 &
echo "GET /ws HTTP/1.1" | nc -w 1 127.0.0.1 18080
# Server prints the handshake-stub diagnostic and exits.
```

## What works end-to-end (v0)

- TCP listener wiring + per-connection callback dispatch.
- Binary-safe recv via `std::io::tcp::Stream.recv_bytes`.
- RFC 6455 frame header parsing — FIN, opcode (text/binary/
  close/ping/pong/continuation), MASK, payload length
  (7-bit, 16-bit, and 64-bit extended forms), 4-byte mask key.
- XOR-unmasking per byte (via 8-bit arithmetic emulation).
- Printable-ASCII payload rendering.
- File-fixture-based parser demo with full asserts.

## What's stubbed (v0 ceiling — see FRICTION.md)

1. **`WsConnL.send_text` / `.send_close` / `.send_pong`** —
   needs `std::bytes::from_int(b: Int)` + `std::bytes::concat
   (a, b)` to construct frame headers with bytes ≥ 0x80.
   Currently emits diagnostic, returns -1.
2. **`WsConnL.do_handshake`** — needs SHA-1, base64, and HTTP
   header access. Currently reads the upgrade bytes (proves
   recv works on the upgrade path) then returns false.
3. **XOR via native `^`** — codegen doesn't lower bitwise
   binops on Int. Workaround: per-bit `ws_xor8` walks 8
   iterations of `% 2`. ~8× the instruction count of a native
   xor.
4. **Client-side build** — needs `std::rand::u32()` for the
   mandatory per-frame mask key. Lowest-priority; server-side
   doesn't need it.

Each of those is one round-trip to the compiler session.
Same shape as `recv_bytes` was before Phase 2g — log entry,
proposal, mechanical C-side body, codegen path-call dispatch.

## Cross-references

- `notes/aperio-friction.md` — global friction log; the four
  ws-echo entries live there with the same dates.
- `notes/codebase-onboarding-design.md` — chat-fanout capstone
  this library underwrites once the four FRICTION items
  resolve.
- `apps/reload-demo/` and `apps/market-book/` — the two prior
  apps in this codebase that exercise the lotus + bus pattern
  on top of currently-shipped primitives. The architecture
  pattern (lotus tower + per-connection let-bound child) is
  mirrored here.
- `crates/aperio-codegen/tests/tcp_recv_bytes.rs` — the Phase
  2g test suite this app builds on top of.
- `crates/aperio-codegen/runtime/stdlib/io_tcp.ap:23-42` —
  `Stream` surface: `send`, `send_bytes`, `recv`, `recv_bytes`.

## The fixture

`fixtures/text-hello-masked.frame` is the binary form of a
single client→server WebSocket text frame containing "hello":

```
81 85           # FIN=1 | opcode=1 (text)  | MASK=1 | len=5
37 fa 21 3d     # 4-byte mask key
5f 9f 4d 51 58  # XOR-masked "hello"
```

After unmasking: `0x5f^0x37=0x68 'h'`, `0x9f^0xfa=0x65 'e'`,
`0x4d^0x21=0x6c 'l'`, `0x51^0x3d=0x6c 'l'`, `0x58^0x37=0x6f
'o'`.

Built once via shell `printf` and checked in. To regenerate
or extend the fixture set, the printf line is in the test's
build history — any reasonable WebSocket client (e.g.
`websocat`, browser DevTools) also produces real frames you
can capture and drop in.
