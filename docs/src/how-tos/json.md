# Read & write JSON

`std::json` is **flat-only at v1** — it covers single-level
objects and top-level arrays of single-level objects. Nested
trees are out of scope. The bet is that wire-format JSON for
most services is flat; if you need a tree, hand-write the
parse or pull in a contrib library.

Two sides:

- **Reading** — flat field lookups + an array iterator over
  the top level.
- **Writing** — a streaming `Builder` that tracks scope state
  for you.

## Reading flat objects

```aperio
let raw = "{\"name\":\"alice\",\"age\":30,\"admin\":true}";
let name  = std::json::find_string_field(raw, "name");      // "alice"
let age   = std::json::find_int_field(raw, "age");          // 30
let admin = std::json::find_bool_field(raw, "admin");       // true

let missing = std::json::find_string_field(raw, "nope");    // ""
let unset   = std::json::find_int_field(raw, "nope");       // 0
```

Missing fields return zero values — `""` for strings, `0` for
ints, `false` for bools. There's no fallible variant; if you
need to distinguish "missing" from "present-and-empty," you
need a richer parser.

## Descending into nested objects

For payloads where the real fields live inside a wrapper object
(`"result":{...}`, `"data":[{...}]` channel state),
`find_field_raw` returns the raw value-token substring:

```aperio
let s = "{\"result\":{\"channel\":\"book\",\"symbol\":\"XBT/USD\"}}";
let inner = std::json::find_field_raw(s, "result");
// inner == `{"channel":"book","symbol":"XBT/USD"}`
let channel = std::json::find_string_field(inner, "channel");
let symbol  = std::json::find_string_field(inner, "symbol");
```

The walker is bracket-balanced over `{...}` and `[...]` (respects
embedded string contents), so the returned substring covers the
whole nested object. Strings come back with their surrounding
quotes preserved (`"alice"` returns `"\"alice\""`); numeric /
boolean / null tokens come back verbatim. Recursive descent: re-
feed the returned substring into any of `find_string_field` /
`find_int_field` / `find_bool_field` / `find_field_raw`.

## Walking top-level arrays

For a top-level JSON array of objects, use `array_first` +
`array_next`:

```aperio
let raw = "[{\"name\":\"alice\",\"age\":30},{\"name\":\"bob\",\"age\":25}]";

let mut it = std::json::array_first(raw);
while !it.done {
    let name = std::json::find_string_field(it.element, "name");
    let age  = std::json::find_int_field(it.element, "age");
    println(name, " is ", to_string(age));
    it = std::json::array_next(it);
}
```

`array_first` returns an `ArrayIter { element, done, ... }`.
Each iteration's `element` is the raw JSON object substring;
hand it to the field readers. The empty array `[]` returns
`done == true` immediately.

## Writing with `Builder`

`Builder` accumulates a JSON document into an internal buffer.
It tracks open-scope state — object vs array, populated vs
empty — and inserts separators (`,` between siblings, `:`
between key and value) automatically.

```aperio
let b = std::json::Builder { };
b.begin_object();
b.field("name", "alice");
b.int_field("age", 30);
b.bool_field("admin", true);
b.begin_array_field("tags");
b.value("ops");
b.value("admin");
b.end_array();
b.end_object();

let out = b.result();
// {"name":"alice","age":30,"admin":true,"tags":["ops","admin"]}
```

The method set:

| Group | Methods |
|---|---|
| Scopes | `begin_object`, `end_object`, `begin_array`, `end_array` |
| Object key+value (one call) | `field` (string), `string_field`, `int_field`, `bool_field`, `null_field` |
| Array entry / bare value | `value` (string), `string_value`, `int_value`, `bool_value`, `null_value` |
| Open a nested scope inside an object | `begin_object_field(name)`, `begin_array_field(name)` |
| Finish | `result() -> String` |

The typed setters (`int_field`, `bool_field`, etc.) are the
clearer choice when the value is anything other than a string
— they encode the value correctly without you reaching for
`to_string` or a JSON-escape pass.

## Escaping raw strings

If you need to emit a raw JSON string outside the Builder
(say, building a request body by hand), use `escape_string`:

```aperio
let safe = std::json::escape_string(user_input);
let body = "{\"comment\":" + safe + "}";       // safe is already wrapped in quotes
```

`escape_string` wraps the input in `"..."` and escapes
characters per RFC 8259. The inverse `unescape_string` takes a
quoted JSON string and returns the unescaped contents.

## A worked example — HTTP server returning JSON

Combine `Builder` with `std::http`:

```aperio
type Item { id: Int; name: String; }

@form(vec)
locus Items {
    capacity { heap data of Item; }
}

locus Routes {
    params { items: Items = Items { }; }

    fn handle(req: std::http::Request) -> std::http::Response {
        if req.method != "GET" || req.path != "/items" {
            return std::http::Response { status: 404, body: "" };
        }

        let b = std::json::Builder { };
        b.begin_array();
        let mut i = 0;
        let total = self.items.len();
        while i < total {
            let item = self.items.get(i) or Item { id: 0, name: "" };
            b.begin_object();
            b.int_field("id", item.id);
            b.field("name", item.name);
            b.end_object();
            i = i + 1;
        }
        b.end_array();

        return std::http::Response {
            status: 200,
            content_type: "application/json",
            body: b.result()
        };
    }
}

fn main() {
    std::http::Server { port: 8080, handler: Routes { } };
}
```

## What `std::json` doesn't do

- **Nested trees.** No `find_*_field` for paths like
  `"user.address.city"`; no walker for arrays-of-arrays. Reach
  for a contrib parser if you need nested traversal.
- **Schema validation.** No "decode this into a `type
  AppConfig`" call. You decode field by field.
- **Pretty-printing.** `Builder.result()` is compact JSON with
  no whitespace.

## See also

- [Build an HTTP server](./http-server.md) — the canonical
  pairing.
- [Standard library](../reference/stdlib.md) — full
  `std::json` surface listing.
