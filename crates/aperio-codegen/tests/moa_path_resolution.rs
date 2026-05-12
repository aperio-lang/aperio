//! moa::* path-resolution tests.
//!
//! Verifies that the `moa::*` top-level path prefix resolves through
//! the codegen's `MOA_PATH_RENAMES` table to the bundled types
//! declared in `moa/types.ap` and concatenated via `MOA_AP_SOURCE`.
//! Parallel to the existing stdlib path-resolution mechanism for
//! `std::*`; this test set provides regression coverage for the
//! second magic prefix.

use std::process::Command;

use aperio_codegen::build_executable;

fn build(name: &str, src: &str) -> std::path::PathBuf {
    let program = aperio_syntax::parse_source(src).expect("parse");
    let mut bin = std::env::temp_dir();
    bin.push(format!("aperio_test_moa_path_{}", name));
    build_executable(&program, &bin).expect("build");
    bin
}

#[test]
fn moa_locus_id_constructs_and_reads_fields() {
    // The simplest possible moa::* path resolution: construct a
    // moa::LocusId literal, read its fields. Verifies path-rename
    // table dispatches `["moa", "LocusId"]` → `__MoaLocusId`, the
    // type registers in user_types, and field access through the
    // struct layout works.
    let src = r#"
        fn main() {
            let id = moa::LocusId {
                name: "BookL",
                path: "apps/market-book/book.ap",
            };
            println(id.name);
            println(id.path);
        }
    "#;
    let bin = build("locus_id_construct", src);
    let out = Command::new(&bin).output().expect("run");
    let _ = std::fs::remove_file(&bin);
    assert!(out.status.success(), "non-zero exit: {:?}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("BookL"),
        "missing locus name in stdout: {:?}",
        stdout
    );
    assert!(
        stdout.contains("apps/market-book/book.ap"),
        "missing path in stdout: {:?}",
        stdout
    );
}

#[test]
fn moa_runtime_event_nests_moa_locus_id() {
    // moa::RuntimeEvent contains a moa::LocusId as a nested field.
    // This verifies that nested-struct payload composition works
    // across two moa::* substrate types (per spec §Phase 2: bus
    // payloads support nested user struct types recursively; same
    // mechanism applies to plain struct literal composition).
    let src = r#"
        fn main() {
            let id = moa::LocusId {
                name: "MdGatewayL",
                path: "apps/market-book/gateway.ap",
            };
            let ev = moa::RuntimeEvent {
                kind: 1,
                origin: id,
                subject: "book.delta",
                payload_size: 64,
                timestamp_ns: 1000000,
            };
            println(ev.subject);
            println(ev.origin.name);
            println(ev.kind);
            println(ev.payload_size);
        }
    "#;
    let bin = build("runtime_event_nests_locus_id", src);
    let out = Command::new(&bin).output().expect("run");
    let _ = std::fs::remove_file(&bin);
    assert!(out.status.success(), "non-zero exit: {:?}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("book.delta"), "subject missing: {:?}", stdout);
    assert!(stdout.contains("MdGatewayL"), "nested locus.name missing: {:?}", stdout);
    assert!(stdout.contains("1"), "kind discriminator missing: {:?}", stdout);
    assert!(stdout.contains("64"), "payload_size missing: {:?}", stdout);
}

#[test]
fn moa_tick_constructs_and_reads() {
    // moa::Tick is the canonical monotonic-clock pulse payload.
    // Two-field type, both Int — simplest possible moa primitive.
    let src = r#"
        fn main() {
            let t = moa::Tick { now_ns: 12345, seq: 7 };
            println(t.now_ns);
            println(t.seq);
        }
    "#;
    let bin = build("tick_construct", src);
    let out = Command::new(&bin).output().expect("run");
    let _ = std::fs::remove_file(&bin);
    assert!(out.status.success(), "non-zero exit: {:?}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("12345"), "now_ns missing: {:?}", stdout);
    assert!(stdout.contains("7"), "seq missing: {:?}", stdout);
}

#[test]
fn moa_braid_id_constructs_and_reads() {
    // moa::BraidId names a bus subscription connection. Three-string
    // type; verifies path resolution for a moa type with no Int
    // fields (all String, exercising the strlen ABI uniformly).
    let src = r#"
        fn main() {
            let b = moa::BraidId {
                subject: "book.delta",
                from_path: "apps/market-book/gateway.ap",
                to_path: "apps/market-book/book.ap",
            };
            println(b.subject);
            println(b.from_path);
            println(b.to_path);
        }
    "#;
    let bin = build("braid_id_construct", src);
    let out = Command::new(&bin).output().expect("run");
    let _ = std::fs::remove_file(&bin);
    assert!(out.status.success(), "non-zero exit: {:?}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("book.delta"), "subject missing: {:?}", stdout);
    assert!(
        stdout.contains("apps/market-book/gateway.ap"),
        "from_path missing: {:?}",
        stdout
    );
}
