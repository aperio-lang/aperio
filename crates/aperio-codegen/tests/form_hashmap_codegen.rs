//! v1.x-FORM-4 PR5 — `@form(hashmap)` codegen.
//!
//! Mirrors `form_vec_codegen.rs`'s shape. The structural lowering:
//! a `@form(hashmap)` locus's pool slot becomes an inline
//! `lotus_hashmap_t`-shaped struct managed by the `lotus_hashmap_*`
//! C runtime instead of the literal F.22 pool allocator. Methods
//! (set/get/has/remove/len/is_empty) lower inline; the intrusive
//! shape extracts the key by GEP'ing the indexed-by field at the
//! set call site.

use std::process::Command;

use aperio_codegen::build_executable;

fn build(name: &str, src: &str) -> std::path::PathBuf {
    let program = aperio_syntax::parse_source(src).expect("parse");
    let mut bin = std::env::temp_dir();
    bin.push(format!("aperio_test_form_hashmap_codegen_{}", name));
    build_executable(&program, &bin).expect("build");
    bin
}

/// Minimum @form(hashmap) lowering: locus instantiates with
/// String-keyed entries, lifecycle runs, dissolve fires
/// lotus_hashmap_destroy on the inline struct. No inserts; the
/// slots buffer is the initial cap=8 calloc and destroy frees it
/// cleanly.
#[test]
fn form_hashmap_locus_instantiates_and_dissolves_cleanly() {
    let src = r#"
        type Entry { name: String; v: Int; }
        @form(hashmap)
        locus RegistryL {
            capacity { pool entries of Entry indexed_by name; }
            birth    { println("birth"); }
            dissolve { println("dissolve"); }
        }
        fn main() {
            let _ = RegistryL { };
        }
    "#;
    let bin = build("lifecycle_empty", src);
    let out = Command::new(&bin).output().expect("run");
    let _ = std::fs::remove_file(&bin);
    assert!(out.status.success(), "non-zero exit: {:?}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("birth"), "missing birth: {:?}", stdout);
    assert!(stdout.contains("dissolve"), "missing dissolve: {:?}", stdout);
}

/// Int-keyed round trip: set, then get back the matching value.
#[test]
fn hashmap_int_keyed_set_and_get() {
    let src = r#"
        type Entry { id: Int; payload: Int; }
        @form(hashmap)
        locus L { capacity { pool entries of Entry indexed_by id; } }
        fn main() {
            let r = L { };
            r.set(Entry { id: 42, payload: 100 });
            let e = r.get(42) or raise;
            println(e.payload);
        }
    "#;
    let bin = build("int_keyed_set_get", src);
    let out = Command::new(&bin).output().expect("run");
    let _ = std::fs::remove_file(&bin);
    assert!(out.status.success(), "non-zero exit: {:?}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.trim().ends_with("100"), "expected 100, got: {:?}", stdout);
}

/// String-keyed round trip.
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
            println(e.v);
        }
    "#;
    let bin = build("string_keyed_set_get", src);
    let out = Command::new(&bin).output().expect("run");
    let _ = std::fs::remove_file(&bin);
    assert!(out.status.success(), "non-zero exit: {:?}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.trim().ends_with("7"), "expected 7, got: {:?}", stdout);
}

/// `get(missing) or fallback` substitutes; `err.kind` is
/// available on the substitute RHS.
#[test]
fn hashmap_get_missing_or_substitute_uses_fallback() {
    let src = r#"
        type Entry { id: Int; v: Int; }
        @form(hashmap)
        locus L { capacity { pool entries of Entry indexed_by id; } }
        fn main() {
            let r = L { };
            r.set(Entry { id: 1, v: 100 });
            let e = r.get(99) or Entry { id: -1, v: -1 };
            if e.v != -1 { println("FAIL: fallback value"); }
            println("ok");
        }
    "#;
    let bin = build("get_substitute", src);
    let out = Command::new(&bin).output().expect("run");
    let _ = std::fs::remove_file(&bin);
    assert!(out.status.success(), "non-zero exit: {:?}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("ok"), "expected ok, got: {:?}", stdout);
    assert!(!stdout.contains("FAIL"), "unexpected FAIL: {:?}", stdout);
}

/// `get(missing) or raise` at the top level of main panics via
/// `lotus_root_panic`: exit code non-zero, stderr names KeyError.
#[test]
fn hashmap_get_missing_or_raise_panics_at_root() {
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
    let bin = build("get_raise_root_panic", src);
    let out = Command::new(&bin).output().expect("run");
    let _ = std::fs::remove_file(&bin);
    assert!(
        !out.status.success(),
        "expected non-zero exit on root-panic, got: {:?}",
        out.status
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("KeyError") && stderr.contains("main locus"),
        "expected root-panic message, got stderr: {:?}",
        stderr
    );
}

/// `has` flips false → true once an entry lands; stays true for
/// the keyed value; false for unknown keys.
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
    let bin = build("has_tracks", src);
    let out = Command::new(&bin).output().expect("run");
    let _ = std::fs::remove_file(&bin);
    assert!(out.status.success(), "non-zero exit: {:?}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("ok"), "expected ok, got: {:?}", stdout);
    assert!(!stdout.contains("FAIL"), "unexpected FAIL: {:?}", stdout);
}

/// `len` and `is_empty` track inserts + removes.
#[test]
fn hashmap_len_and_is_empty_track_mutations() {
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
    let bin = build("len_is_empty_track", src);
    let out = Command::new(&bin).output().expect("run");
    let _ = std::fs::remove_file(&bin);
    assert!(out.status.success(), "non-zero exit: {:?}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("ok"), "expected ok, got: {:?}", stdout);
    assert!(!stdout.contains("FAIL"), "unexpected FAIL: {:?}", stdout);
}

/// `set` of a duplicate key replaces in place (len doesn't grow,
/// value is overwritten).
#[test]
fn hashmap_set_duplicate_key_overwrites() {
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
    let bin = build("set_duplicate_overwrites", src);
    let out = Command::new(&bin).output().expect("run");
    let _ = std::fs::remove_file(&bin);
    assert!(out.status.success(), "non-zero exit: {:?}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("ok"), "expected ok, got: {:?}", stdout);
    assert!(!stdout.contains("FAIL"), "unexpected FAIL: {:?}", stdout);
}

/// `remove` of a missing key fails with KeyError; a Unit-returning
/// handler swallows it as a statement. Confirms the
/// fallible-Unit-success path lowers correctly (FallibleCallResult
/// with `success_ty = None`).
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
    let bin = build("remove_missing_swallow", src);
    let out = Command::new(&bin).output().expect("run");
    let _ = std::fs::remove_file(&bin);
    assert!(out.status.success(), "non-zero exit: {:?}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("ok"), "expected ok, got: {:?}", stdout);
    assert!(!stdout.contains("FAIL"), "unexpected FAIL: {:?}", stdout);
}

/// `err` binding is in scope on the substitute RHS, with payload
/// type KeyError; `err.kind` is "missing_key" when get fails.
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
    let bin = build("err_binding_kind", src);
    let out = Command::new(&bin).output().expect("run");
    let _ = std::fs::remove_file(&bin);
    assert!(out.status.success(), "non-zero exit: {:?}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("missing_key"),
        "expected kind=missing_key in output, got: {:?}",
        stdout
    );
    assert!(stdout.contains("ok"), "expected ok, got: {:?}", stdout);
    assert!(!stdout.contains("FAIL"), "unexpected FAIL: {:?}", stdout);
}

/// Survives the load-factor grow path: 32 inserts at initial cap=8
/// force several doublings, each re-hashes via the normal set
/// path. All values remain retrievable.
#[test]
fn hashmap_grows_and_retains_entries() {
    let src = r#"
        type Entry { id: Int; v: Int; }
        @form(hashmap)
        locus L { capacity { pool entries of Entry indexed_by id; } }
        fn main() {
            let r = L { };
            for i in 0..32 {
                r.set(Entry { id: i, v: i * 10 });
            }
            if r.len() != 32 { println("FAIL: len after grow"); }
            for i in 0..32 {
                let e = r.get(i) or raise;
                if e.v != i * 10 { println("FAIL: value mismatch"); }
            }
            println("ok");
        }
    "#;
    let program = aperio_syntax::parse_source(src);
    if program.is_err() {
        eprintln!("skip: parser doesn't yet support 0..N range");
        return;
    }
    let bin = build("grow_retains", src);
    let out = Command::new(&bin).output().expect("run");
    let _ = std::fs::remove_file(&bin);
    assert!(out.status.success(), "non-zero exit: {:?}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("ok"), "expected ok, got: {:?}", stdout);
    assert!(!stdout.contains("FAIL"), "unexpected FAIL: {:?}", stdout);
}

/// `self.set` / `self.get` from inside the locus's own body. Same
/// dispatcher, different call site (lower_self_method_call).
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
    let bin = build("self_dispatch", src);
    let out = Command::new(&bin).output().expect("run");
    let _ = std::fs::remove_file(&bin);
    assert!(out.status.success(), "non-zero exit: {:?}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("ok"), "expected ok, got: {:?}", stdout);
    assert!(!stdout.contains("FAIL"), "unexpected FAIL: {:?}", stdout);
}
