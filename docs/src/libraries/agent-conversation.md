<!-- Synced from hale-lang/pond/agent/conversation/README.md by tools/sync-pond-docs.sh — do not edit here. -->

# pond/agent/conversation — alias `conv`

Chat-history locus for AI-agent pipelines. Holds an ordered
stream of `Message` records bounded by `max_messages`, and
surfaces the state both as a tab-separated string (for direct
hand-off to an LLM request) and as opaque `Bytes` (for snapshot
+ delta-shipping via the MOA substrate).

```hale
import "vendor/pond/agent/conversation" as conv;

let c = conv::Conversation {
    system_prompt: "You are terse.",
    max_messages:  10,
    last_error:    conv::ConversationError { },
};

c.append(conv::Message {
    role:    "user",
    content: "hello",
    ts:      `2026-05-16T00:00:00Z`,
});

let hist = c.history();          // "user\thello"
let snap = c.snapshot_bytes();   // for MOA delta-shipping
```

## Public surface

Per `pond/CONTRACTS.md § pond/agent/conversation/`.

| Member                | Shape                                                  |
|-----------------------|--------------------------------------------------------|
| `Message`             | type — `{ role: String; content: String; ts: Time; }` |
| `ConversationError`   | type — `{ kind: String; }`                             |
| `ConvVersion`         | type — `{ generation: Int; era: Int; }` (MoaOwner)     |
| `DecodedDelta`        | type — `{ kind, system, history, pair_count }`         |
| `Conversation`        | locus — see below                                      |
| `ConversationUpdated` | topic — wire subject `"conv.updated"`, payload `Message` |
| `decode_delta`        | free fn — `(d: Bytes) -> DecodedDelta fallible(ConversationError)` |

### `Conversation` locus

```hale
locus Conversation {
    params {
        system_prompt: String = "";
        max_messages:  Int    = 100;
        // ...internal state fields (see conversation.hl)
        last_error:    ConversationError;   // required at construction
    }
    bus { publish ConversationUpdated; }

    fn append(m: Message);                 // record + fire topic
    fn history() -> String;                // tab-separated dump
    fn version() -> ConvVersion;           // MoaOwner surface
    fn snapshot_bytes() -> Bytes;          // MoaOwner surface
    fn apply_delta(d: Bytes);              // MoaOwner surface (non-fallible bridge)
    fn last_error_kind() -> String;        // post-apply_delta inspection
}
```

The `last_error` param is REQUIRED at construction (no default —
because `b""` / empty-struct param defaults trip the lexer
gotcha `pond/moa::InMemoryStore.initial` runs into; the same
shape applies here). Construct as
`last_error: conv::ConversationError { }`.

## MoaOwner relationship

`Conversation` structurally satisfies `pond/moa::MoaOwner`:

```hale
interface MoaOwner {
    fn version() -> Version;        // ↔ Conversation.version() -> ConvVersion
    fn snapshot_bytes() -> Bytes;   // ↔ Conversation.snapshot_bytes()
    fn apply_delta(d: Bytes) -> (); // ↔ Conversation.apply_delta(d)
}
```

The structural-satisfaction check (per `spec/types.md` § Interface
types) is method-set-superset: the names, arg types, and return
types of the three methods match the interface verbatim, so any
consumer that holds a `MoaOwner` reference can pass a
`Conversation`.

**One caveat** (logged in FRICTION.md): `Conversation.version()`
returns a local `ConvVersion` shape rather than `moa::Version`
to avoid forcing every consumer of `pond/agent/conversation` to
also vendor `pond/moa` (pond/README.md § Design rules rule 4).
`ConvVersion` is field-for-field identical to `moa::Version`
(`generation: Int`, `era: Int`), so structural compatibility holds
when both libs are imported into the same seed.

The standard usage shape, then, is to vendor BOTH libs at the
app level:

```hale
import "vendor/pond/moa"               as moa;
import "vendor/pond/agent/conversation" as conv;

let c = conv::Conversation { /* ... */ };

// `c` satisfies moa::MoaOwner — pass it anywhere a moa::MoaOwner
// reference is expected (e.g. a delta-shipping consumer's
// configured owner ref).
```

A consumer (`moa::MoaConsumer`) subscribes to the bus topic
`conv::ConversationUpdated` to mirror state incrementally, and
calls `apply_conversation_delta(consumer_local_conv, delta_bytes)`
when applying a wire delta on the consumer side. The
snapshot-bytes round-trip preserves both the system prompt and
the full message history.

## Wire shapes

### `Conversation.history()` and `Message.content`

`history()` returns `"role1\tcontent1\trole2\tcontent2\t..."` —
tab-separated `role\tcontent` pairs flattened into one stream.
Same convention as `pond/agent/llm::LlmRequest.messages` and
`pond/router::RouteParams.path_kv`; a Conversation's `history()`
output drops straight into an `LlmRequest`:

```hale
let req = llm::LlmRequest {
    model:    "claude-opus-4-7",
    system:   c.system_prompt,
    messages: c.history(),
    // ...
};
```

Callers MUST NOT embed `\t` in `Message.content` — there is no
escaping at v1. (Logged in FRICTION.md as duplicate-suspected
with the same constraint in `pond/router` and `pond/agent/llm`.)

### `snapshot_bytes()` / `apply_delta()` wire shape

```
<system_prompt>\n<history_buf>
```

The system prompt up front, a single LF, then the tab-separated
history stream. Round-trip:

```hale
let snap = owner_conv.snapshot_bytes();
apply_conversation_delta(consumer_conv, snap) or raise;
// consumer_conv now mirrors owner_conv's full state.
```

For incremental deltas (single new message), the wire shape is
just `"role\tcontent"` (no LF, no system prompt) — `apply_delta`
detects the snapshot vs delta shape by presence of an LF in the
bytes.

### `ConversationError` kinds

| `kind`           | Meaning                                                                                  |
|------------------|------------------------------------------------------------------------------------------|
| `""`             | success (`last_error` cleared on every successful `apply_delta`)                         |
| `"empty_delta"`  | `apply_delta` got zero bytes                                                             |
| `"role_missing"` | delta had an odd field count (a role with no content)                                    |
| `"decode_failed"`| reserved for future binary-delta shapes; not produced by the tab-stream decoder          |

## Pattern-catalog mapping

`Conversation` is a **Service locus** (pattern 3 in the
six-pattern catalog) — state-bearing, exposes a method surface
that mutates `self`, fires bus events on mutation. No explicit
`birth()` / `run()` / `dissolve()` because every param has either
a default or a required-at-construction value; state is usable
immediately.

`decode_delta` and the small helpers (`count_messages`,
`drop_front_messages`, `count_tabs`, `count_message_pairs_or_zero`)
are **free fns** (pattern 6); the fallible decode path lives
there because locus methods can't declare `fallible(E)` per the
two-channel rule (G4). The locus's `apply_delta` method calls
`decode_delta` and folds the resulting `DecodedDelta` value
onto its own fields. See FRICTION.md for the `self`-in-arg
codegen gotcha that forced the "pure decode + fold" split.

## Examples

```bash
hale build pond/agent/conversation/examples/two-turn/
./examples/two-turn/two-turn
```

The example constructs a `Conversation`, appends "hello",
"hi there", "how are you", snapshots the bytes, and prints the
history.

## Cross-references

- `pond/CONTRACTS.md` — the binding surface this lib targets.
- `pond/agent/conversation/FRICTION.md` — deviations + gaps.
- `pond/moa/README.md` (when written) — the MoaOwner interface
  this lib structurally satisfies.
- `pond/KNOWN_GOTCHAS.md` § G4 — the two-channel rule that
  forces `apply_delta`'s fallible surface into a free fn.
- `pond/agent/llm/README.md` — the natural downstream consumer
  of `history()`.
