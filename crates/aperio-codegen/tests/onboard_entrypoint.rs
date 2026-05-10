//! Entrypoint-mode rendering — outward + inward towers.
//!
//! When the scanned dir has `func main()` somewhere, onboard
//! renders two trees rooted at that entrypoint: the outward
//! call graph and the inward import graph (rerooted at main's
//! file). These tests pin the substrings each tower should
//! produce against `apps/operational-graph/fixture` so any
//! regression in the entrypoint walker or its renderers
//! surfaces fast. Substring matching, not line-exact — the
//! tower shape is the contract; whitespace and decoration
//! are free to evolve.

use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use aperio_codegen::build_executable;

fn workspace_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.pop();
    p
}

fn run_against_operational_fixture() -> String {
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
        "aperio_onboard_entrypoint_{}_{}",
        std::process::id(),
        nanos
    ));
    build_executable(&program, &bin).expect("build onboard");
    let fixture = workspace_root()
        .join("apps")
        .join("operational-graph")
        .join("fixture");
    let out = Command::new(&bin)
        .arg(fixture)
        .output()
        .expect("run onboard");
    let _ = std::fs::remove_file(&bin);
    assert!(
        out.status.success(),
        "onboard exited non-zero: {:?}; stderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).to_string()
}

#[test]
fn outward_tower_announces_section_header() {
    let report = run_against_operational_fixture();
    assert!(
        report.contains("Outward tower"),
        "missing outward tower header; output:\n{}",
        report
    );
    assert!(
        report.contains("what main() does at runtime"),
        "missing outward subtitle; output:\n{}",
        report
    );
}

#[test]
fn outward_tower_roots_at_main_in_main_file() {
    let report = run_against_operational_fixture();
    // The root row carries the entrypoint name, the file it
    // lives in, and a domain motion-form attached via "·".
    assert!(
        report.contains("main()  (main.go)  · "),
        "expected `main()  (main.go)  · <motion>` root; output:\n{}",
        report
    );
}

#[test]
fn outward_tower_recurses_into_in_package_callees() {
    let report = run_against_operational_fixture();
    // backgroundWorker is defined in worker.go; main() spawns
    // it, so the outward walker resolves the callee via FN_DEF
    // and renders the file annotation.
    assert!(
        report.contains("backgroundWorker  (worker.go)"),
        "expected backgroundWorker rendered with worker.go; output:\n{}",
        report
    );
    // fanout is called from backgroundWorker — depth-2 recursion.
    assert!(
        report.contains("fanout  (worker.go)"),
        "expected fanout rendered (depth-2 from main); output:\n{}",
        report
    );
}

#[test]
fn outward_tower_terminates_at_external_leaves() {
    let report = run_against_operational_fixture();
    // ListenAndServe is net/http — no FN_DEF in scope — so it
    // renders as {external} and the walker stops there.
    assert!(
        report.contains("ListenAndServe  {external}"),
        "expected ListenAndServe rendered as {{external}}; output:\n{}",
        report
    );
    // NewTicker is time — same shape.
    assert!(
        report.contains("NewTicker  {external}"),
        "expected NewTicker rendered as {{external}}; output:\n{}",
        report
    );
}

#[test]
fn outward_tower_attaches_domain_motion_at_each_node() {
    let report = run_against_operational_fixture();
    // Suffix-rule hits: backgroundWorker has "Worker" → "working"
    // via the -er rule. The full motion shows the CamelCase
    // composition.
    assert!(
        report.contains("backgroundWorker"),
        "expected backgroundWorker present; output:\n{}",
        report
    );
    assert!(
        report.contains("working"),
        "expected `working` motion to surface somewhere in outward; output:\n{}",
        report
    );
}

#[test]
fn inward_tower_announces_section_header() {
    let report = run_against_operational_fixture();
    assert!(
        report.contains("Inward tower"),
        "missing inward tower header; output:\n{}",
        report
    );
    assert!(
        report.contains("what main()'s package needs"),
        "missing inward subtitle; output:\n{}",
        report
    );
}

#[test]
fn inward_tower_lists_main_file_first_with_its_imports() {
    let report = run_against_operational_fixture();
    // The main.go block comes before the others so main's own
    // imports are read top-down.
    let inward_start = report
        .find("Inward tower")
        .expect("inward header present");
    let inward = &report[inward_start..];
    let main_pos = inward.find("main.go\n").expect("main.go block");
    let handlers_pos = inward.find("handlers.go\n");
    let worker_pos = inward.find("worker.go\n");
    assert!(
        handlers_pos.map(|h| h > main_pos).unwrap_or(true),
        "main.go should appear before handlers.go in inward tower; output:\n{}",
        inward
    );
    assert!(
        worker_pos.map(|w| w > main_pos).unwrap_or(true),
        "main.go should appear before worker.go in inward tower; output:\n{}",
        inward
    );
    assert!(
        inward.contains("├─ log  {stdlib}"),
        "expected log classified as stdlib; output:\n{}",
        inward
    );
    assert!(
        inward.contains("├─ net/http  {stdlib}"),
        "expected net/http classified as stdlib; output:\n{}",
        inward
    );
}

#[test]
fn inward_tower_seeds_with_wire_target_files() {
    let report = run_against_operational_fixture();
    // helloHandler / statusHandler live in handlers.go and are
    // registered via mux.HandleFunc. main() never calls them
    // directly, so without the WIRE seed they'd be invisible to
    // the call-graph walk. The inward tower must still list
    // handlers.go because it's needed at runtime.
    let inward_start = report.find("Inward tower").expect("inward header");
    let inward = &report[inward_start..];
    assert!(
        inward.contains("handlers.go"),
        "expected handlers.go in inward tower (reached via WIRE); \
         output:\n{}",
        inward
    );
    // handlers.go imports fmt and net/http; the classification
    // and presence are the contract.
    assert!(
        inward.contains("├─ fmt  {stdlib}"),
        "expected handlers.go's fmt import; output:\n{}",
        inward
    );
}

/// Stage a tiny multi-dir Go fixture so cross-dir resolution
/// has something to chase. Layout:
///
///   <root>/go.mod              `module xdir`
///   <root>/cmd/api/main.go     calls Init + Run
///   <root>/internal/setup.go   `Init` defined here, calls `Connect`
///   <root>/internal/db.go      `Connect` defined here
fn stage_xdir_fixture() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut root = std::env::temp_dir();
    root.push(format!(
        "aperio_onboard_xdir_{}_{}",
        std::process::id(),
        nanos
    ));
    std::fs::create_dir_all(root.join("cmd/api")).unwrap();
    std::fs::create_dir_all(root.join("internal")).unwrap();
    std::fs::write(root.join("go.mod"), "module xdir\n").unwrap();
    std::fs::write(
        root.join("cmd/api/main.go"),
        "package main\n\
         \n\
         import (\n\
         \t\"xdir/internal\"\n\
         )\n\
         \n\
         func main() {\n\
         \tinternal.Init()\n\
         \tinternal.Run()\n\
         }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("internal/setup.go"),
        "package internal\n\
         \n\
         func Init() {\n\
         \tConnect()\n\
         }\n\
         \n\
         func Run() {\n\
         \tConnect()\n\
         }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("internal/db.go"),
        "package internal\n\
         \n\
         func Connect() {\n\
         }\n",
    )
    .unwrap();
    root
}

#[test]
fn cross_dir_resolves_callees_into_sibling_pkg() {
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
        "aperio_onboard_xdir_bin_{}_{}",
        std::process::id(),
        nanos
    ));
    build_executable(&program, &bin).expect("build");
    let root = stage_xdir_fixture();
    let out = Command::new(&bin)
        .arg(root.join("cmd/api"))
        .output()
        .expect("run");
    let _ = std::fs::remove_file(&bin);
    let _ = std::fs::remove_dir_all(&root);
    assert!(
        out.status.success(),
        "onboard exited non-zero on xdir fixture: {:?}; stderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let report = String::from_utf8_lossy(&out.stdout).to_string();
    // The outward tower should show main → Init resolved to a
    // sibling-dir file (internal/setup.go), not {external}.
    assert!(
        report.contains("Init  (internal/setup.go)"),
        "expected main → Init resolved to internal/setup.go; \
         output:\n{}",
        report
    );
    // And Run resolved to its sibling file too.
    assert!(
        report.contains("Run  (internal/setup.go)"),
        "expected main → Run resolved to internal/setup.go; \
         output:\n{}",
        report
    );
    // Depth-2 cross-dir: Init calls Connect, which lives in
    // internal/db.go.
    assert!(
        report.contains("Connect  (internal/db.go)"),
        "expected depth-2 cross-dir resolution to \
         internal/db.go; output:\n{}",
        report
    );
    // Inward tower marks xdir/internal as {local}, not stdlib.
    assert!(
        report.contains("xdir/internal  {local}"),
        "expected xdir/internal classified as {{local}}; \
         output:\n{}",
        report
    );
}

#[test]
fn no_go_mod_means_no_cross_dir_walk() {
    // Operational fixture has no go.mod next to it. The
    // entrypoint walk should still work, but no sibling dirs
    // get pulled in — and no callee should resolve to a path
    // containing a "/" (the cross-dir file-stamp marker).
    let report = run_against_operational_fixture();
    let outward_start = report
        .find("Outward tower")
        .expect("outward header present");
    let inward_start = report
        .find("Inward tower")
        .expect("inward header present");
    let outward = &report[outward_start..inward_start];
    // None of the resolved file-stamps in the outward tower
    // should have a "/" — only flat filenames like worker.go.
    for line in outward.lines() {
        // file annotations look like `(filename)`; skip
        // anything that isn't a resolved-callee row.
        if let Some(open) = line.find('(') {
            if let Some(close) = line[open..].find(')') {
                let stamp = &line[open + 1..open + close];
                assert!(
                    !stamp.contains('/'),
                    "no-go.mod run should not emit slash-bearing \
                     file stamps in outward tower; got `{}` on line: {}",
                    stamp,
                    line
                );
            }
        }
    }
}

#[test]
fn entrypoint_mode_skipped_when_no_main_present() {
    // operational fixture has main; check the library-package
    // path stays alphabetical-only by running against the
    // morpheme test corpus (store.go-only, no main).
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
        "aperio_onboard_no_main_{}_{}",
        std::process::id(),
        nanos
    ));
    build_executable(&program, &bin).expect("build");
    // Stage a temp dir with only store.go (no main).
    let mut dir = std::env::temp_dir();
    dir.push(format!(
        "aperio_onboard_no_main_fixture_{}_{}",
        std::process::id(),
        nanos
    ));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let store = workspace_root()
        .join("apps")
        .join("operational-graph")
        .join("fixture")
        .join("store.go");
    std::fs::copy(&store, dir.join("store.go")).expect("copy store.go");
    let out = Command::new(&bin)
        .arg(&dir)
        .output()
        .expect("run");
    let _ = std::fs::remove_file(&bin);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        out.status.success(),
        "onboard exited non-zero on no-main fixture: {:?}; stderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let report = String::from_utf8_lossy(&out.stdout).to_string();
    // Neither tower header should appear when there's no main.
    assert!(
        !report.contains("Outward tower"),
        "outward tower must not render without a main; output:\n{}",
        report
    );
    assert!(
        !report.contains("Inward tower"),
        "inward tower must not render without a main; output:\n{}",
        report
    );
    // But the per-file render still happens.
    assert!(
        report.contains("StoreL"),
        "per-file pass should still run on no-main dir; output:\n{}",
        report
    );
}
