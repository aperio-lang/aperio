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

Three deviations land in this implementation; see `FRICTION.md`
for the full audit. Brief summary:

- **`register(t: Tool)` is split into `register(e: Entry)` plus
  convenience free fns `register_tool` / `register_fns`.**
  Interface values can't sit in `@form(vec)` cells at v1
  (spec/types.md § F.20 Phase B; `KNOWN_GOTCHAS.md` § G20). The
  fn-pointer-shadow approach matches what `pond/router` and
  `pond/jobs` already shipped for the same gap.
- **`dispatch(call) -> ToolResult fallible(ToolError)` is split
  into `Registry.dispatch_call(call) -> ToolResult` (non-fallible
  method, returns an `is_error` ToolResult on miss) plus a
  fallible free fn `tools::dispatch(reg, call) -> ToolResult
  fallible(ToolError)`.** Per the two-channel rule
  (`KNOWN_GOTCHAS.md` § G4), user-declared locus methods may not
  declare `fallible(E)`. Both paths share the same lookup kernel.
- **Cross-seed consumers must use `Registry` methods, not the
  convenience free fns.** Per `KNOWN_GOTCHAS.md` § G11, calls
  like `tools::register_tool(reg, ...)` from a consumer seed
  don't lower at v1. The free fns stay declared in `registry.ap`
  for in-seed callers and as the unblock-day surface; consumer
  code uses `reg.register(tools::Entry { spec, invoke_fn })` and
  `reg.dispatch_call(call)` instead.

## Writing a Tool

A Tool is any locus exposing the two interface methods (for
forward compatibility once F.20 Phase B unblocks) plus a pair
of top-level free-fn shadows for the v1 fn-pointer storage path.

```aperio
import "vendor/pond/agent/tools" as tools;

// Free fns are the v1 registry-facing surface; the locus
// methods stay in-shape with the Tool interface for the
// Phase-B-unblock future.
fn calc_spec() -> tools::ToolSpec {
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

fn calc_invoke(call: tools::ToolCall) -> tools::ToolResult {
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

// Tool-interface-shaped locus. The methods aren't called by the
// Registry at v1 (the fn-pointer pair above is what gets stored
// + dispatched), but consumers writing code against the `Tool`
// interface in fn signatures will pick this up structurally.
locus Calculator {
    params { }
    fn spec()   -> tools::ToolSpec { return calc_spec(); }
    fn invoke(call: tools::ToolCall) -> tools::ToolResult {
        return calc_invoke(call);
    }
}
```

Register and dispatch (cross-seed-consumer v1 shape):

```aperio
let reg = tools::Registry { };

// Build the Entry literal at the call site (cross-seed free-fn
// path calls don't lower at v1 — KNOWN_GOTCHAS.md § G11).
reg.register(tools::Entry {
    spec:      calc_spec(),
    invoke_fn: calc_invoke
});

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
raise` is callable cross-seed (works through the fallible-`or`
codegen path):

```aperio
let result = tools::dispatch(reg, call) or raise;
```

The non-fallible `tools::register_tool(reg, spec, invoke_fn)`
compiles but currently segfaults at runtime when called
cross-seed (see `FRICTION.md` `cross-seed-locus-arg-segv`); use
`reg.register(tools::Entry { ... })` until the upstream gap
closes.

## Pattern catalog mapping

- `Registry`     — pattern 3 (service locus). Implicit lifecycle;
  the `EntryList` child storage births / dissolves with it.
- `EntryList`    — pattern 3 backing storage (`@form(vec)` child).
- `Tool`         — F.20 interface (forward-compat).
- `Entry`        — pattern 5 (shape type). Internal storage cell.
- `ToolSpec / ToolCall / ToolResult / ToolError` — pattern 5
  shape types; the public wire surface.
- `dispatch / register_tool / register_fns` — pattern 6 free fns.
  Free because lifecycle methods can't declare `fallible(E)`
  (two-channel rule) and because fn-pointer registration is
  naturally a non-method shape.

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
  Wrap with a free-fn pair the way Calculator does.

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
