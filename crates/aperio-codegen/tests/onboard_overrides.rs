//! Per-codebase overrides — agentic feedback loop.
//!
//! An LLM agent reads `apps/onboard`'s output, classifies
//! flagged unknowns by reading source, and writes its decisions
//! to `<dir>/.aperio-overrides`. Subsequent runs honor those
//! decisions without contaminating the universal seed lookup
//! in `std::lang::Morpheme`.
//!
//! These tests stage the loop end-to-end: copy a known fixture
//! into a temp dir, drop in an overrides file with known
//! resolutions, run onboard, assert the motion-forms reflect
//! the overrides and the unknowns section drops to zero.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use aperio_codegen::build_executable;

fn workspace_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.pop();
    p
}

fn build_onboard() -> PathBuf {
    let src_path = workspace_root()
        .join("apps")
        .join("onboard")
        .join("main.ap");
    let src = std::fs::read_to_string(&src_path).expect("read main.ap");
    let program = aperio_syntax::parse_source(&src).expect("parse main.ap");
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut bin = std::env::temp_dir();
    bin.push(format!(
        "aperio_onboard_overrides_{}_{}",
        std::process::id(),
        nanos
    ));
    build_executable(&program, &bin).expect("build onboard");
    bin
}

fn make_temp_fixture() -> PathBuf {
    // Stage a copy of operational-graph/fixture/store.go in a
    // temp dir. store.go is the only file with type
    // declarations the morpheme rewriter sees, so it's enough
    // to exercise the overrides path.
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut dir = std::env::temp_dir();
    dir.push(format!(
        "aperio_overrides_fixture_{}_{}",
        std::process::id(),
        nanos
    ));
    std::fs::create_dir_all(&dir).expect("mkdir fixture");
    let store = workspace_root()
        .join("apps")
        .join("operational-graph")
        .join("fixture")
        .join("store.go");
    std::fs::copy(&store, dir.join("store.go")).expect("copy store.go");
    dir
}

fn run(bin: &Path, fixture: &Path) -> String {
    let out = Command::new(bin)
        .arg(fixture)
        .output()
        .expect("run onboard");
    assert!(
        out.status.success(),
        "onboard exited non-zero: {:?}; stderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).to_string()
}

#[test]
fn overrides_resolve_unknown_morphemes_in_motion_forms() {
    let bin = build_onboard();
    let fixture = make_temp_fixture();
    // Without overrides, the rewriter marks Request/Session/Audit
    // as <unknown:X>.
    let before = run(&bin, &fixture);
    assert!(
        before.contains("<unknown:Request>-remembering"),
        "expected unknown Request marker pre-overrides; output:\n{}",
        before
    );
    assert!(
        before.contains("<unknown:Session>-managing"),
        "expected unknown Session marker pre-overrides; output:\n{}",
        before
    );

    // Drop in an overrides file resolving all three.
    let overrides = "\
# Agent-resolved per this codebase.
Request: requesting
Session: tracking
Audit: auditing
";
    std::fs::write(fixture.join(".aperio-overrides"), overrides)
        .expect("write overrides");

    let after = run(&bin, &fixture);
    // Resolved motions appear.
    assert!(
        after.contains("RequestCache → requesting-remembering"),
        "expected resolved RequestCache motion; output:\n{}",
        after
    );
    assert!(
        after.contains("SessionManager → tracking-managing"),
        "expected resolved SessionManager motion; output:\n{}",
        after
    );
    assert!(
        after.contains("AuditLogger → auditing-logging"),
        "expected resolved AuditLogger motion; output:\n{}",
        after
    );
    // Unknown markers are gone.
    assert!(
        !after.contains("<unknown:Request>"),
        "regression: unknown Request marker remains after override; output:\n{}",
        after
    );

    let _ = std::fs::remove_file(&bin);
    let _ = std::fs::remove_dir_all(&fixture);
}

#[test]
fn overrides_header_announces_load() {
    let bin = build_onboard();
    let fixture = make_temp_fixture();
    std::fs::write(
        fixture.join(".aperio-overrides"),
        "Request: requesting\n",
    )
    .expect("write overrides");

    let report = run(&bin, &fixture);
    assert!(
        report.contains("overrides: .aperio-overrides loaded"),
        "expected overrides-loaded header line; output:\n{}",
        report
    );

    let _ = std::fs::remove_file(&bin);
    let _ = std::fs::remove_dir_all(&fixture);
}

#[test]
fn overrides_tolerate_comments_and_blank_lines() {
    let bin = build_onboard();
    let fixture = make_temp_fixture();
    let overrides = "\
# This is a comment line — ignored.

# Blank line above is also ignored.
Request: requesting

# Comments can appear between entries.
Session: tracking
Audit: auditing
# Trailing comment.
";
    std::fs::write(fixture.join(".aperio-overrides"), overrides)
        .expect("write overrides");

    let report = run(&bin, &fixture);
    assert!(
        report.contains("requesting-remembering"),
        "expected Request resolved despite comments/blanks; output:\n{}",
        report
    );
    assert!(
        report.contains("tracking-managing"),
        "expected Session resolved; output:\n{}",
        report
    );

    let _ = std::fs::remove_file(&bin);
    let _ = std::fs::remove_dir_all(&fixture);
}

#[test]
fn overrides_tolerate_whitespace_around_colon() {
    let bin = build_onboard();
    let fixture = make_temp_fixture();
    // Mix of formats to exercise the trim() fallback.
    let overrides = "\
Request:requesting
Session : tracking
Audit:    auditing
";
    std::fs::write(fixture.join(".aperio-overrides"), overrides)
        .expect("write overrides");

    let report = run(&bin, &fixture);
    for resolved in ["requesting-remembering", "tracking-managing", "auditing-logging"] {
        assert!(
            report.contains(resolved),
            "expected {} resolved despite whitespace; output:\n{}",
            resolved, report
        );
    }

    let _ = std::fs::remove_file(&bin);
    let _ = std::fs::remove_dir_all(&fixture);
}

#[test]
fn missing_overrides_file_is_silent() {
    // When no .aperio-overrides exists, the report should have
    // no "overrides: loaded" line and the rewriter behaves like
    // the seed-only path.
    let bin = build_onboard();
    let fixture = make_temp_fixture();
    // Don't write an overrides file.
    let report = run(&bin, &fixture);
    assert!(
        !report.contains("overrides:"),
        "expected silent header without overrides file; output:\n{}",
        report
    );

    let _ = std::fs::remove_file(&bin);
    let _ = std::fs::remove_dir_all(&fixture);
}
