<!-- Synced from hale-lang/pond/agent/llm/README.md by tools/sync-pond-docs.sh — do not edit here. -->

# pond/agent/llm — Anthropic + OpenAI HTTP clients with SSE streaming

Suggested import alias: **`llm`**

```hale
import "vendor/pond/agent/llm" as llm;
```

## Status (2026-05-18)

The library builds clean and the public surface matches
`pond/CONTRACTS.md § pond/agent/llm/`. One load-bearing caveat
remains (eager-buffering on the streaming path); the prior TLS
gap closed 2026-05-18 with `std::io::tls::*` shipping upstream
and the source-level wire-up landing in this commit.

### TLS — `api.anthropic.com` over HTTPS now dials directly

`base_url` accepts both `http://` and `https://` schemes; the
URL parser routes the right scheme into the right substrate
(`std::io::tls::connect` for `https`, `std::io::tcp::connect`
for `http`). No config flag — just point at the real endpoint:

```hale
let client = llm::AnthropicClient {
    api_key:  std::env::var("ANTHROPIC_API_KEY"),
    base_url: "https://api.anthropic.com"    // default — TLS by scheme
};
```

The TLS handshake uses SNI + system trust store (TLS 1.2+,
`SSL_VERIFY_PEER`). Build-time link drags in `-lssl -lcrypto`
automatically — no extra steps. Local HTTP-only endpoints
(LM Studio, llama.cpp, Ollama) still work as before; point
`base_url` at `http://localhost:1234` to dial plaintext.

### Eager-buffering on the streaming path

The streaming surface (`AnthropicClient.stream` /
`OpenAiClient.stream`) drains the entire HTTP response off the
socket *before* walking the SSE frames and firing per-chunk
`LlmChunk` topics. A true low-latency client would feed each
`recv_bytes` chunk into the buffer and publish chunks as they
arrive; the v1 shape sacrifices that for a simpler control
flow (one round-trip drain, then one pass over the body).

For short prompts the difference is invisible. For long
generations the user sees nothing until the whole response is
in memory, then receives every chunk in a burst. Logged as a
follow-up in `FRICTION.md`.

## Public surface

Per CONTRACTS.md (`pond/CONTRACTS.md § pond/agent/llm/`):

```hale
type LlmRequest  { model: String; system: String; messages: String;
                   max_tokens: Int; temperature: Float; }
type LlmResponse { text: String; stop_reason: String;
                   input_tokens: Int; output_tokens: Int; }
type LlmError    { kind: String; status: Int; detail: String; }

locus AnthropicClient {
    params { api_key: String; base_url: String = "https://api.anthropic.com";
             default_model: String = "claude-opus-4-7"; }
    fn complete(req: LlmRequest) -> LlmResponse;        // see deviation note
    fn stream(req: LlmRequest);                         // see deviation note
    bus { publish "agent.llm.chunk" of type LlmChunk;
          publish "agent.llm.done"  of type LlmDone; }
}

locus OpenAiClient {
    params { api_key: String; base_url: String = "https://api.openai.com";
             default_model: String = "gpt-4o"; }
    fn complete(req: LlmRequest) -> LlmResponse;        // see deviation note
    fn stream(req: LlmRequest);                         // see deviation note
    bus { publish "agent.llm.chunk" of type LlmChunk;
          publish "agent.llm.done"  of type LlmDone; }
}

type LlmChunk { payload: String;      }
type LlmDone  { payload: LlmResponse; }

// Free-fn surface — same shapes, with the value-channel
// `fallible(LlmError)` annotation that locus methods can't carry.
fn anthropic_complete(api_key, base_url, req, max_body)
    -> LlmResponse fallible(LlmError);
fn openai_complete(api_key, base_url, req, max_body)
    -> LlmResponse fallible(LlmError);
```

### Two-channel deviation

Per `spec/semantics.md § Fallible call semantics`, user-declared
locus methods cannot declare `fallible(E)`. CONTRACTS.md lists
`complete()` and `stream()` as locus methods with fallible
returns; the implementation deviates in the standard way (see
`pond/subprocess/process.hl`, `pond/http/client/client.hl` —
same pattern across pond):

- Methods are non-fallible. They wrap the matching free-fn
  kernel (`anthropic_complete`, `openai_complete`,
  `__anthropic_fetch_sse`, `__openai_fetch_sse`) with the
  standard `or self.__record(err)` clause from
  `spec/styleguide.md § 7`.
- The captured error is readable through
  `client.last_error_kind()`, `client.last_error_status()`,
  and `client.last_error_detail()`. A successful call leaves
  `last_error_kind()` returning `""`.
- Consumers that want hard fallible semantics call the free
  fns directly: `let r = llm::anthropic_complete(key, url,
  req, max_body) or raise;`.

### Bus subjects

Both clients publish on the **topic-ident** form (canonical
per spec/semantics.md § Topic declarations):
`topic LlmChunk { payload: LlmChunkMsg; subject:
"agent.llm.chunk"; }` and `topic LlmDone { payload: LlmDoneMsg;
subject: "agent.llm.done"; }`. The bus requires a user-defined
type at every publish site, so each topic wraps its payload in
a thin `*Msg` struct.

Subscribers wire up by topic ident (no `of type T` — the
topic carries the payload type):

```hale
locus Listener {
    bus {
        subscribe llm::LlmChunk as on_chunk;
        subscribe llm::LlmDone  as on_done;
    }
    fn on_chunk(c: llm::LlmChunkMsg) {
        print(c.payload);
    }
    fn on_done(d: llm::LlmDoneMsg) {
        println("[stop=", d.payload.stop_reason, "]");
    }
}
```

## Example usage

```hale
import "vendor/pond/agent/llm" as llm;

locus Talker {
    run() {
        let client = llm::AnthropicClient {
            api_key:       std::env::var("ANTHROPIC_API_KEY"),
            base_url:      "http://localhost:1234"    // proxy
        };
        let req = llm::LlmRequest {
            model:       "claude-opus-4-7",
            system:      "You are terse.",
            messages:    "user\tSay hello in 3 words.",
            max_tokens:  64,
            temperature: 0.7
        };
        let resp = client.complete(req);
        if len(client.last_error_kind()) > 0 {
            println("error: ", client.last_error_detail());
        } else {
            println(resp.text);
        }
    }
}

fn main() { Talker { }; }
```

The `messages` field is tab-separated `"role\tcontent\trole\t..."`
because Hale v1 has no parametric `List<T>` (same convention
`pond/router`'s `RouteParams.path_kv` and `pond/agent/conversation`
use for the same reason).

## Files

| File              | Contents                                      |
|-------------------|-----------------------------------------------|
| `types.hl`        | `LlmRequest`, `LlmResponse`, `LlmError`       |
| `sse.hl`          | SSE frame buffer + per-vendor delta extractors |
| `wire.hl`         | JSON body builders + response parsers          |
| `anthropic.hl`    | `AnthropicClient` locus + free-fn kernels      |
| `openai.hl`       | `OpenAiClient` locus + free-fn kernels         |
| `wire_topics.hl`  | `LlmChunk` / `LlmDone` topic decls + `LlmChunkMsg` / `LlmDoneMsg` payload wrappers |

## Demo

```bash
$ hale build \
      pond/agent/llm/examples/echo-completion/
$ ./examples/echo-completion/echo-completion
echo-completion: dialing http://localhost:1234
echo-completion: model    claude-opus-4-7
echo-completion: prompt   Say hello in 3 words.
echo-completion: error kind   = http
echo-completion: error status = 0
echo-completion: error detail = connect failed: localhost:1234
echo-completion: (see README for the local-proxy / HTTPS workaround)
```

Without a live proxy on `localhost:1234` the demo reports
`connect_failed` and exits — exactly as documented. Run an
LM Studio (or equivalent) server on that port to see the
real round-trip path.

Override via env vars:

- `ANTHROPIC_API_KEY` — sets the `x-api-key` header.
- `PROXY_BASE_URL` — overrides `http://localhost:1234`.
- `LLM_MODEL` — overrides the default model.

## Verification

```bash
$ hale build \
      pond/agent/llm/
codegen error: unsupported in codegen v0: program has no `fn main()`
```

The lib type-checks cleanly; the "no main" message is
expected for a library directory (matches every other pond
lib's build behavior — see `pond/subprocess/`'s same shape).
End-to-end verification is via the example:

```bash
$ hale build \
      pond/agent/llm/examples/echo-completion/
built: .../examples/echo-completion/echo-completion
```

## Dependencies

Per `pond/README.md`'s no-transitive-deps rule, this lib in
principle depends on `pond/http/client` (alias `http`) for
URL parsing + the HTTP request/response shapes. In practice
`pond/http/client` does not currently build cleanly (a
parallel build issue — see `FRICTION.md § dependency-on-http-
client`), so the lib reaches into `std::io::tcp::*` and
`std::str::index_of` directly, with an inline URL parser
(`anthropic.hl § __parse_base_url`). Once http/client lands
the inline parser should fold back into `http::parse_url`.
