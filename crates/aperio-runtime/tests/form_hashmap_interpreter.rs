//! v1.x-FORM-4 PR6: end-to-end `@form(hashmap)` execution under
//! the interpreter. Mirrors `form_vec_interpreter.rs` shape and
//! covers the same surface as `crates/aperio-codegen/tests/
//! form_hashmap_codegen.rs` so AOT (codegen) and JIT (interpreter)
//! agree on observable behavior.

use aperio_runtime::run_program;

fn run(src: &str) -> i32 {
    let program = aperio_syntax::parse_source(src)
        .map_err(|d| {
            d.iter()
                .map(|x| x.render(src))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .expect("parse");
    run_program(&program).expect("run")
}

fn run_expect_error(src: &str) -> String {
    let program = aperio_syntax::parse_source(src)
        .map_err(|d| {
            d.iter()
                .map(|x| x.render(src))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .expect("parse");
    match run_program(&program) {
        Ok(_) => panic!("expected program to exit with an error, got ok"),
        Err(s) => s,
    }
}

#[test]
fn hashmap_int_keyed_set_and_get_round_trip() {
    let src = r#"
        type Entry { id: Int; payload: Int; }
        @form(hashmap)
        locus L { capacity { pool entries of Entry indexed_by id; } }
        fn main() {
            let r = L { };
            r.set(Entry { id: 42, payload: 100 });
            let e = r.get(42) or raise;
            if e.payload != 100 { println("FAIL: payload"); }
            println("ok");
        }
    "#;
    assert_eq!(run(src), 0);
}

#[test]
fn hashmap_string_keyed_set_and_get() {
    let src = r#"
        type Entry { name: String; v: Int; }
        @form(hashmap)
        locus L { capacity { pool entries of Entry indexed_by name; } }
        fn main() {
            let r = L { };
            r.set(Entry { name: "alpha", v: 7 });
            let e = r.get("alpha") or raise;
            if e.v != 7 { println("FAIL: value"); }
            println("ok");
        }
    "#;
    assert_eq!(run(src), 0);
}

#[test]
fn hashmap_get_missing_substitute_uses_fallback() {
    let src = r#"
        type Entry { id: Int; v: Int; }
        @form(hashmap)
        locus L { capacity { pool entries of Entry indexed_by id; } }
        fn main() {
            let r = L { };
            r.set(Entry { id: 1, v: 100 });
            let e = r.get(99) or Entry { id: -1, v: -1 };
            if e.v != -1 { println("FAIL: fallback"); }
            println("ok");
        }
    "#;
    assert_eq!(run(src), 0);
}

#[test]
fn hashmap_get_missing_or_raise_panics() {
    let src = r#"
        type Entry { id: Int; v: Int; }
        @form(hashmap)
        locus L { capacity { pool entries of Entry indexed_by id; } }
        fn main() {
            let r = L { };
            let e = r.get(99) or raise;
            println(e.v);
        }
    "#;
    // The unaddressed `raise` past the top-level fn surfaces as
    // a runtime error in the interpreter (no on_failure to
    // catch). Just verify the program errors out rather than
    // succeeding — mirroring the vec analogue's shape.
    let _err = run_expect_error(src);
}

#[test]
fn hashmap_has_tracks_sets() {
    let src = r#"
        type Entry { id: Int; v: Int; }
        @form(hashmap)
        locus L { capacity { pool entries of Entry indexed_by id; } }
        fn main() {
            let r = L { };
            if r.has(1) { println("FAIL: has before set"); }
            r.set(Entry { id: 1, v: 100 });
            if !r.has(1) { println("FAIL: has after set"); }
            if r.has(99) { println("FAIL: has unknown"); }
            println("ok");
        }
    "#;
    assert_eq!(run(src), 0);
}

#[test]
fn hashmap_len_and_is_empty_track() {
    let src = r#"
        type Entry { id: Int; v: Int; }
        @form(hashmap)
        locus L { capacity { pool entries of Entry indexed_by id; } }
        fn main() {
            let r = L { };
            if !r.is_empty() { println("FAIL: not initially empty"); }
            if r.len() != 0 { println("FAIL: initial len"); }
            r.set(Entry { id: 1, v: 1 });
            r.set(Entry { id: 2, v: 2 });
            r.set(Entry { id: 3, v: 3 });
            if r.is_empty() { println("FAIL: empty after sets"); }
            if r.len() != 3 { println("FAIL: len after sets"); }
            r.remove(2) or raise;
            if r.len() != 2 { println("FAIL: len after remove"); }
            println("ok");
        }
    "#;
    assert_eq!(run(src), 0);
}

#[test]
fn hashmap_set_duplicate_overwrites() {
    let src = r#"
        type Entry { id: Int; v: Int; }
        @form(hashmap)
        locus L { capacity { pool entries of Entry indexed_by id; } }
        fn main() {
            let r = L { };
            r.set(Entry { id: 1, v: 100 });
            r.set(Entry { id: 1, v: 200 });
            if r.len() != 1 { println("FAIL: len grew on duplicate"); }
            let e = r.get(1) or raise;
            if e.v != 200 { println("FAIL: old value remains"); }
            println("ok");
        }
    "#;
    assert_eq!(run(src), 0);
}

#[test]
fn hashmap_remove_missing_substitute_swallows() {
    let src = r#"
        type Entry { id: Int; v: Int; }
        @form(hashmap)
        locus L { capacity { pool entries of Entry indexed_by id; } }
        fn ignore(_e: KeyError) { }
        fn main() {
            let r = L { };
            r.set(Entry { id: 1, v: 1 });
            r.remove(99) or ignore(err);
            if r.len() != 1 { println("FAIL: live entry removed"); }
            println("ok");
        }
    "#;
    assert_eq!(run(src), 0);
}

#[test]
fn hashmap_err_binding_kind_available() {
    let src = r#"
        type Entry { id: Int; v: Int; }
        @form(hashmap)
        locus L { capacity { pool entries of Entry indexed_by id; } }
        fn fallback(e: KeyError) -> Entry {
            println(e.kind);
            return Entry { id: -1, v: -1 };
        }
        fn main() {
            let r = L { };
            let e = r.get(42) or fallback(err);
            if e.v != -1 { println("FAIL: fallback not used"); }
            println("ok");
        }
    "#;
    assert_eq!(run(src), 0);
}

#[test]
fn hashmap_self_dispatch_inside_locus_method() {
    let src = r#"
        type Entry { id: Int; v: Int; }
        @form(hashmap)
        locus L {
            capacity { pool entries of Entry indexed_by id; }
            fn seed() {
                self.set(Entry { id: 1, v: 100 });
                self.set(Entry { id: 2, v: 200 });
            }
        }
        fn main() {
            let r = L { };
            r.seed();
            let a = r.get(1) or raise;
            let b = r.get(2) or raise;
            if a.v + b.v != 300 { println("FAIL: sum"); }
            println("ok");
        }
    "#;
    assert_eq!(run(src), 0);
}

#[test]
fn hashmap_many_entries_survive_lookup() {
    // Interpreter doesn't have a grow concept — backing is
    // Vec<(K,V)> with linear scan — but verify that many entries
    // round-trip cleanly.
    let src = r#"
        type Entry { id: Int; v: Int; }
        @form(hashmap)
        locus L { capacity { pool entries of Entry indexed_by id; } }
        fn main() {
            let r = L { };
            for i in 0..16 {
                r.set(Entry { id: i, v: i * 10 });
            }
            if r.len() != 16 { println("FAIL: len after sets"); }
            for i in 0..16 {
                let e = r.get(i) or raise;
                if e.v != i * 10 { println("FAIL: value mismatch"); }
            }
            println("ok");
        }
    "#;
    assert_eq!(run(src), 0);
}

#[test]
fn hashmap_empty_get_or_raise_errors() {
    let src = r#"
        type Entry { name: String; v: Int; }
        @form(hashmap)
        locus L { capacity { pool entries of Entry indexed_by name; } }
        fn main() {
            let r = L { };
            let _ = r.get("anything") or raise;
            println("unreachable");
        }
    "#;
    let _ = run_expect_error(src);
}
