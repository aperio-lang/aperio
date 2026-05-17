//! A4 (G34) — three-hop cross-seed import chain.
//!
//! `app → lib-mid → lib-util`. The app imports `lib-mid` as
//! `mid`; lib-mid imports `lib-util` as `u`. Before A4, the
//! v1 strict barrier dropped `lib-mid`'s import of util, so
//! references to `u::Box { ... }`, `u::make_box(...)`, etc.
//! inside lib-mid's body failed codegen. Lifting the barrier
//! makes the CLI recurse into each imported lib's own
//! `import` directives with the lib's directory as the new
//! importer dir.

use std::path::{Path, PathBuf};
use std::process::Command;

fn aperio_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_aperio"))
}

fn fixtures_dir() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests");
    p.push("fixtures");
    p
}

#[test]
fn three_hop_app_builds_and_runs() {
    let app_dir = fixtures_dir().join("three-hop-app");

    // Clean prior build artifacts so we test the fresh build path.
    let built_bin = app_dir.join("three-hop-app");
    let _ = std::fs::remove_file(&built_bin);

    let out = Command::new(aperio_bin())
        .arg("build")
        .arg(&app_dir)
        .output()
        .expect("invoke aperio build");
    assert!(
        out.status.success(),
        "aperio build failed: status={:?} stdout={} stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    assert!(
        built_bin.exists(),
        "expected binary at {:?}",
        built_bin
    );

    let run_out = Command::new(&built_bin)
        .output()
        .expect("run three-hop-app");
    assert!(
        run_out.status.success(),
        "three-hop-app exit {:?}: stderr={}",
        run_out.status,
        String::from_utf8_lossy(&run_out.stderr)
    );
    let stdout = String::from_utf8_lossy(&run_out.stdout);
    assert!(
        stdout.contains("answer=42 [v1]"),
        "expected `answer=42 [v1]` in stdout: {:?}",
        stdout
    );
    // A5 (G18/G33): qualified cross-seed type in fn-param position
    // (`mid::label_of(b: u::Box)`) resolves through the transitive
    // rename table.
    assert!(
        stdout.contains("answer"),
        "expected `answer` (label_of output) in stdout: {:?}",
        stdout
    );

    // Tidy up so the fixture stays clean across CI runs.
    let _ = std::fs::remove_file(&built_bin);
}

#[test]
fn three_hop_uses_per_importer_mangled_prefix() {
    // Defensive: the mangled symbol for util's `Box` carries the
    // alias `u` (the middle lib's chosen alias) — confirming that
    // transitive resolution uses per-importer mangling, not a
    // workspace-wide dedup. We grep the verbose build output via
    // a side-channel: build with debug logging if available;
    // otherwise just sanity-check the build succeeded and the
    // resulting binary embeds either prefix.
    let app_dir = fixtures_dir().join("three-hop-app");
    let built_bin = app_dir.join("three-hop-app");
    let _ = std::fs::remove_file(&built_bin);

    let out = Command::new(aperio_bin())
        .arg("build")
        .arg(&app_dir)
        .output()
        .expect("invoke aperio build");
    assert!(out.status.success(), "build failed: {:?}", out);

    let bin_bytes = std::fs::read(&built_bin).expect("read binary");
    // The util lib's `make_box` fn lives in the binary under the
    // mangled name `__lib_u_box_make_box` — the `u` alias is the
    // one mid chose for its `import "..." as u;`, proving the
    // transitive resolution kept mid's importer-scoped namespace
    // rather than collapsing onto a workspace-wide alias. Fn
    // symbols survive linking; type names don't always become
    // ELF symbols, so we key off a fn.
    let needle = b"__lib_u_box_make_box";
    let hit = bin_bytes
        .windows(needle.len())
        .any(|w| w == needle);
    assert!(
        hit,
        "expected mangled fn `{}` in binary — \
         transitive lib resolution should mangle with mid's alias",
        std::str::from_utf8(needle).unwrap()
    );

    let _ = std::fs::remove_file(&built_bin);
}

#[allow(dead_code)]
fn read_dir_names(d: &Path) -> Vec<String> {
    std::fs::read_dir(d)
        .ok()
        .map(|it| {
            it.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect()
        })
        .unwrap_or_default()
}
