<!-- Synced from aperio-lang/pond/agent/tools/README.md by tools/sync-pond-docs.sh — do not edit here. -->

# pond/agent/tools — Tool registry for LLM tool-use

Suggested alias: `tools`. Vendored as
`import "vendor/pond/agent/tools" as tools;`.

The registry is the wire-shape glue between an LLM client (e.g.
`pond/agent/llm::AnthropicClient`) and a set of side-effecting
tools the LLM is allowed to invoke. Each tool publishes a
`ToolSpec` (name + description + JSON-schema for its args); the
client posts the array of specs to the vendor; the LLM picks one
and emits a `ToolCall`; the Registry dispatches the call to the
matching tool's invoke; the resulting `ToolResult` rides back to
the LLM in the next turn.

## Surface (per `pond/CONTRACTS.md § pond/agent/tools/`)

```aperio
type ToolSpec  { name: String; description: String; input_schema: String; }
type ToolCall  { name: String; args_json: String; call_id: String; }
type ToolResult { call_id: String; content: String; is_error: Bool; }
type ToolError  { kind: String; detail: String; }

interface Tool {
    fn spec() -> ToolSpec;
    fn invoke(call: ToolCall) -> ToolResult;
}

locus Registry {
    params { }
    fn register(t: Tool) -> ();
    fn dispatch(call: ToolCall) -> ToolResult fallible(ToolError);
    fn list() -> String;
}
```

## v1 deviations

One deviation remains; see `FRICTION.md` for the full audit.

- **`dispatch(call) -> ToolResult fallible(ToolError)` is split
  into `Registry.dispatch_call(call) -> ToolResult` (non-fallible
  method, returns an `is_error` ToolResult on miss) plus a
  fallible free fn `tools::dispatch(reg, call) -> ToolResult
  fallible(ToolError)`.** Per the two-channel rule
  (`KNOWN_GOTCHAS.md` § G4), user-declared locus methods may not
  declare `fallible(E)`. Both paths share the same lookup kernel.

The previous fn-pointer/`Entry`-wrapper deviation is gone:
F.20 Phase B (G20) shipped interface values in `@form(vec)`
cells, and the Registry now stores `Tool` directly. Cross-seed
consumers register via the free fn `tools::register(reg, t)`
(user-declared locus method arg coercion `LocusRef → Interface`
isn't yet wired across seeds; the free-fn arg site is the wired
coercion path).

## Writing a Tool

A Tool is any locus whose method set structurally satisfies the
`Tool` interface — declare `spec()` and `invoke()` directly on
the locus; no fn-pointer shadow needed.

```aperio
import "vendor/pond/agent/tools" as tools;

locus Calculator {
    params { }

    fn spec() -> tools::ToolSpec {
        return tools::ToolSpec {
            name:        "calculator",
            description: "Evaluate a simple arithmetic expression.",
            input_schema:
                "{\"type\":\"object\","
                + "\"properties\":{"
                + "\"op\":{\"type\":\"string\"},"
                + "\"a\":{\"type\":\"number\"},"
                + "\"b\":{\"type\":\"number\"}},"
                + "\"required\":[\"op\",\"a\",\"b\"]}"
        };
    }

    fn invoke(call: tools::ToolCall) -> tools::ToolResult {
        let op = std::json::find_string_field(call.args_json, "op");
        let a  = std::json::find_int_field(call.args_json, "a");
        let b  = std::json::find_int_field(call.args_json, "b");
        let mut out = "";
        let mut err = false;
        if op == "add"      { out = to_string(a + b); }
        else if op == "sub" { out = to_string(a - b); }
        else if op == "mul" { out = to_string(a * b); }
        else if op == "div" {
            if b == 0 { out = "division by zero"; err = true; }
            else      { out = to_string(a / b);                }
        }
        else { out = "unknown op: " + op; err = true; }
        return tools::ToolResult {
            call_id:  call.call_id,
            content:  out,
            is_error: err
        };
    }
}
```

Register and dispatch (cross-seed consumer shape):

```aperio
let reg  = tools::Registry { };
let calc = Calculator { };

// Free-fn arg site coerces Calculator → Tool; the Registry's
// @form(vec) of Tool stores the fat-pointer directly.
tools::register(reg, calc);

let call = tools::ToolCall {
    name:      "calculator",
    args_json: "{\"op\":\"add\",\"a\":2,\"b\":3}",
    call_id:   "call_001"
};

// Non-fallible method: a miss surfaces as a ToolResult with
// is_error: true and content "unknown_tool: <name>".
let result = reg.dispatch_call(call);
println(result.content);   // "5"

// Or emit the JSON spec array for an LLM tool-use call:
let specs_json = reg.list();
// → [{"name":"calculator","description":"...","input_schema":{...}}]
```

The fallible-channel free fn `tools::dispatch(reg, call) or
raise` is the value-channel surface:

```aperio
let result = tools::dispatch(reg, call) or raise;
```

In-seed callers can also use the locus-method form
`reg.register(calc)` directly. Cross-seed consumers should
prefer `tools::register(reg, calc)` because user-declared
locus-method arg coercion `LocusRef → Interface` isn't yet
wired across seeds (the free-fn arg site is wired).

## Pattern catalog mapping

- `Registry`     — pattern 3 (service locus). Implicit lifecycle;
  the `ToolList` child storage births / dissolves with it.
- `ToolList`     — pattern 3 backing storage (`@form(vec)` child,
  cells of `Tool` interface).
- `Tool`         — F.20 interface. The storage cell type as well
  as the public registration surface.
- `ToolSpec / ToolCall / ToolResult / ToolError` — pattern 5
  shape types; the public wire surface.
- `dispatch / register` — pattern 6 free fns. Free because
  lifecycle methods can't declare `fallible(E)` (two-channel
  rule) and because cross-seed locus-method arg coercion
  `LocusRef → Interface` isn't yet wired (free-fn arg site is).

## Cross-lib pairings

- **`pond/agent/llm`** consumes `Registry.list()` output as the
  `tools: [...]` array fed to Anthropic/OpenAI. The vendor
  responds with a `tool_use` content block; the LLM consumer
  unpacks it into a `ToolCall` and routes through
  `tools::dispatch`.
- **`pond/agent/conversation`** stores the `ToolResult` back
  into the conversation history as the next turn's content.
- **`pond/agent/sandbox`** is a natural `Tool` (its
  `run_code(code)` shape maps to `invoke({"code": "..."})`).
  Declare `spec()` / `invoke()` on the sandbox locus the way
  Calculator does and pass it through `tools::register`.

## Example

`examples/calc-tool/main.ap` — registers a Calculator tool with
add / sub / mul / div, dispatches a sample call
`{"op":"add","a":2,"b":3}`, asserts the result is `"5"`, and
also exercises the `list()` JSON-array spec dump.

Run from the example directory:

```sh
aperio run \
    pond/agent/tools/examples/calc-tool/
```
