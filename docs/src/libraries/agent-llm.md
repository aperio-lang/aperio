<!-- Synced from aperio-lang/pond/agent/llm/README.md by tools/sync-pond-docs.sh — do not edit here. -->

# pond/agent/llm — Anthropic + OpenAI HTTP clients with SSE streaming

Suggested import alias: **`llm`**

```aperio
import "vendor/pond/agent/llm" as llm;
```

## Status (2026-05-16)

The library builds clean and the public surface matches
`pond/CONTRACTS.md § pond/agent/llm/`. There are TWO load-bearing
caveats consumers must understand before reaching for it; both
are documented in `FRICTION.md` with the full design rationale.

### 1. No TLS — `api.anthropic.com` over HTTPS won't dial

The Aperio stdlib (`std::io::tcp::*`) does not currently expose
a TLS implementation. Both clients will happily build
`POST /v1/messages` wire-format requests with the right headers
(`x-api-key`, `anthropic-version`, `Authorization: Bearer ...`,
`Content-Type: application/json`, etc.), but they can only ship
those requests over plaintext TCP. Pointing `base_url` at
`https://api.anthropic.com` (or `https://api.openai.com`) will
fail with `kind: "unsupported_scheme"` from `parse_url`.

**Workarounds** (any one of the three is fine):

- **Local LLM-API-compatible endpoint** — point `base_url` at a
  local server speaking the OpenAI or Anthropic wire format.
  Examples: [LM Studio](https://lmstudio.ai)'s OpenAI-compatible
  server (default `http://localhost:1234`), `llama.cpp`'s
  built-in OpenAI server, [Ollama](https://ollama.ai) with
  `--openai-host`. The demo defaults to
  `http://localhost:1234`, matching LM Studio's out-of-the-box
  setup.
- **HTTP proxy with upstream TLS termination** — run a tiny
  reverse proxy (nginx, Caddy, `socat`, `mitmproxy`,
  `cloudflared`) that listens on plain HTTP locally and
  terminates TLS upstream against the vendor's API.
- **Wait for stdlib TLS** — once `std::io::tls::*` ships
  (substrate roadmap, no firm date) the `unsupported_scheme`
  guard flips off and the same code dials the real endpoint.

### 2. Eager-buffering on the streaming path

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

```aperio
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
`pond/subprocess/process.ap`, `pond/http/client/client.ap` —
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

Both clients publish on **literal-subject** wire-format strings
(`"agent.llm.chunk"` / `"agent.llm.done"`) — see `FRICTION.md
§ topic-rename-asymmetry` for the workaround driving this.
Subscribers wire up by literal subject + explicit payload type:

```aperio
locus Listener {
    bus {
        subscribe "agent.llm.chunk" as on_chunk
            of type llm::LlmChunk;
        subscribe "agent.llm.done"  as on_done
            of type llm::LlmDone;
    }
    fn on_chunk(c: llm::LlmChunk) {
        print(c.payload);
    }
    fn on_done(d: llm::LlmDone) {
        println("[stop=", d.payload.stop_reason, "]");
    }
}
```

## Example usage

```aperio
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
because Aperio v1 has no parametric `List<T>` (same convention
`pond/router`'s `RouteParams.path_kv` and `pond/agent/conversation`
use for the same reason).

## Files

| File              | Contents                                      |
|-------------------|-----------------------------------------------|
| `types.ap`        | `LlmRequest`, `LlmResponse`, `LlmError`       |
| `sse.ap`          | SSE frame buffer + per-vendor delta extractors |
| `wire.ap`         | JSON body builders + response parsers          |
| `anthropic.ap`    | `AnthropicClient` locus + free-fn kernels      |
| `openai.ap`       | `OpenAiClient` locus + free-fn kernels         |
| `wire_topics.ap`  | `LlmChunk` / `LlmDone` payload types          |

## Demo

```bash
$ aperio build \
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
$ aperio build \
      pond/agent/llm/
codegen error: unsupported in codegen v0: program has no `fn main()`
```

The lib type-checks cleanly; the "no main" message is
expected for a library directory (matches every other pond
lib's build behavior — see `pond/subprocess/`'s same shape).
End-to-end verification is via the example:

```bash
$ aperio build \
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
(`anthropic.ap § __parse_base_url`). Once http/client lands
the inline parser should fold back into `http::parse_url`.
