//! brained F.1 — qualified-path cell type in @form(hashmap).
//!
//! Pre-fix: `pool entries of model::Project indexed_by id`
//! errored with "@form(hashmap) cell type must be a user-
//! declared struct" because the typechecker's cell-type
//! extraction only admitted single-segment paths. The
//! codegen-side path-renames table resolved qualified paths
//! at call sites but ran AFTER typecheck — the @form check
//! never saw the resolved name.
//!
//! Post-fix: `apply_qualified_path_renames` rewrites every
//! `TypeExpr::Named` with a multi-segment path to the
//! matching mangled single name BEFORE typecheck runs.
//! Qualified cell types now resolve the same way bare ones
//! do.

use std::path::PathBuf;
use std::process::Command;

fn hale_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_hale"))
}

fn fixtures_dir() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests");
    p.push("fixtures");
    p
}

#[test]
fn form_hashmap_qualified_cell_type_builds_and_runs() {
    let app_dir = fixtures_dir().join("form-hashmap-qualified-app");
    let built_bin = app_dir.join("form-hashmap-qualified-app");
    let _ = std::fs::remove_file(&built_bin);

    let out = Command::new(hale_bin())
        .arg("build")
        .arg(&app_dir)
        .output()
        .expect("invoke hale build");
    assert!(
        out.status.success(),
        "hale build failed: status={:?} stdout={} stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );

    let run_out = Command::new(&built_bin)
        .output()
        .expect("run form-hashmap-qualified-app");
    let _ = std::fs::remove_file(&built_bin);
    assert!(
        run_out.status.success(),
        "binary exit {:?}: stderr={}",
        run_out.status,
        String::from_utf8_lossy(&run_out.stderr)
    );
    let stdout = String::from_utf8_lossy(&run_out.stdout).to_string();
    assert!(
        stdout.contains("p2 / beta"),
        "expected 'p2 / beta' in stdout; got: {}",
        stdout
    );
}
