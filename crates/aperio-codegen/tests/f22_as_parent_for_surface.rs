//! v1.x-4: F.22 `as_parent_for ChildL` slot trailing clause —
//! language surface only at v1. Typecheck validates the
//! reference; codegen explicitly rejects with a "v1.x-4b
//! pending" diagnostic so users don't get silent
//! miscompilation. The runtime mechanic (parent's allocator
//! handed to the child at accept-time + skip-destroy on
//! borrowed slots) ships in v1.x-4b.

use aperio_codegen::build_executable;

#[test]
fn as_parent_for_parses() {
    // The clause parses cleanly; the failure mode at v1 is
    // codegen-side, not parse-side.
    let src = r#"
        locus ChildL {
            capacity { pool entries of Int; }
        }
        locus ParentL {
            capacity {
                pool entries of Int as_parent_for ChildL;
            }
            accept(c: ChildL) { }
        }
        fn main() { }
    "#;
    let program = aperio_syntax::parse_source(src).expect("parse");
    let mut bin = std::env::temp_dir();
    bin.push("aperio_test_f22_apf_parses");
    // Codegen rejection is the v1 outcome — verify the
    // diagnostic names v1.x-4b.
    let err = build_executable(&program, &bin)
        .expect_err("v1 codegen should reject as_parent_for");
    let msg = format!("{}", err);
    assert!(
        msg.contains("v1.x-4b") || msg.contains("as_parent_for"),
        "expected v1.x-4b deferral diagnostic, got: {}",
        msg
    );
}

#[test]
fn as_parent_for_validates_child_locus_exists() {
    // Typecheck should reject naming a locus that doesn't
    // exist. Note: typecheck runs before codegen, so the
    // build error here is the typecheck diagnostic, not the
    // v1.x-4b codegen reject.
    let src = r#"
        locus ParentL {
            capacity {
                pool entries of Int as_parent_for NonExistentL;
            }
        }
        fn main() { }
    "#;
    let program = aperio_syntax::parse_source(src).expect("parse");
    let mut bin = std::env::temp_dir();
    bin.push("aperio_test_f22_apf_missing");
    let err = build_executable(&program, &bin)
        .expect_err("should reject missing-locus");
    let msg = format!("{}", err);
    assert!(
        msg.contains("NonExistentL"),
        "expected diagnostic mentioning NonExistentL, got: {}",
        msg
    );
}

#[test]
fn as_parent_for_validates_child_has_matching_slot() {
    // Child must have a slot of the same name.
    let src = r#"
        locus ChildL {
            capacity { pool other of Int; }
        }
        locus ParentL {
            capacity {
                pool entries of Int as_parent_for ChildL;
            }
            accept(c: ChildL) { }
        }
        fn main() { }
    "#;
    let program = aperio_syntax::parse_source(src).expect("parse");
    let mut bin = std::env::temp_dir();
    bin.push("aperio_test_f22_apf_mismatched_slot");
    let err = build_executable(&program, &bin)
        .expect_err("should reject mismatched slot");
    let msg = format!("{}", err);
    assert!(
        msg.contains("no slot named") || msg.contains("entries"),
        "expected mismatched-slot diagnostic, got: {}",
        msg
    );
}
