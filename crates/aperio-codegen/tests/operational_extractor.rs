//! m100 v0: operational extractor end-to-end test.
//!
//! Builds `apps/operational-graph/main.ap`, runs it against the
//! checked-in fixture (`apps/operational-graph/fixture/`), and
//! asserts on the JSON tower's operational sections. Substring-
//! based assertions rather than full JSON parsing — the v0
//! tower has a fixed shape and the goal is to catch regressions
//! in detection heuristics, not validate JSON conformance.

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

fn build_extractor() -> PathBuf {
    let src_path = workspace_root()
        .join("apps")
        .join("operational-graph")
        .join("main.ap");
    let src = std::fs::read_to_string(&src_path).expect("read main.ap");
    let program = aperio_syntax::parse_source(&src).expect("parse main.ap");
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut bin = std::env::temp_dir();
    bin.push(format!(
        "aperio_operational_{}_{}",
        std::process::id(),
        nanos
    ));
    build_executable(&program, &bin).expect("build extractor");
    bin
}

fn run_against_fixture() -> String {
    let bin = build_extractor();
    let fixture = workspace_root()
        .join("apps")
        .join("operational-graph")
        .join("fixture");
    let out = Command::new(&bin)
        .arg(fixture)
        .output()
        .expect("run extractor");
    let _ = std::fs::remove_file(&bin);
    assert!(
        out.status.success(),
        "extractor exited non-zero: {:?}; stderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).to_string()
}

#[test]
fn finds_main_entrypoint() {
    let json = run_against_fixture();
    assert!(
        json.contains("\"main\": [{\"file\": \"main.go\"}]"),
        "expected main in main.go; output:\n{}",
        json
    );
}

#[test]
fn finds_init_birth_function() {
    let json = run_against_fixture();
    assert!(
        json.contains("\"init\": [{\"file\": \"main.go\"}]"),
        "expected init() in main.go; output:\n{}",
        json
    );
}

#[test]
fn detects_http_handlers_by_param_shape() {
    // Both handlers in handlers.go have the
    // `(w http.ResponseWriter, r *http.Request)` signature; the
    // ResponseWriter+Request substring-match heuristic should
    // catch them both, regardless of order.
    let json = run_against_fixture();
    assert!(
        json.contains("\"name\": \"helloHandler\""),
        "missing helloHandler; output:\n{}",
        json
    );
    assert!(
        json.contains("\"name\": \"statusHandler\""),
        "missing statusHandler; output:\n{}",
        json
    );
    // Both should be in handlers.go — not the worker or main.
    let handlers_section_start = json.find("\"handlers\":").expect("handlers section");
    let handlers_section_end = json[handlers_section_start..]
        .find("],")
        .map(|e| handlers_section_start + e)
        .unwrap_or(json.len());
    let section = &json[handlers_section_start..handlers_section_end];
    assert_eq!(
        section.matches("handlers.go").count(),
        2,
        "expected exactly 2 handlers.go entries in handlers section; got: {}",
        section
    );
}

#[test]
fn distinguishes_named_from_anonymous_goroutines() {
    // The fixture has:
    //   - main.go: `go backgroundWorker()`              (named)
    //   - worker.go: `go fanout()` inside select        (named)
    //   - worker.go: `go func() { ... }()` inside fanout (anonymous)
    let json = run_against_fixture();
    // Find the goroutines section and verify both kinds appear.
    let gor_start = json.find("\"goroutines\":").expect("goroutines section");
    let gor_end = json[gor_start..]
        .find("],")
        .map(|e| gor_start + e)
        .unwrap_or(json.len());
    let section = &json[gor_start..gor_end];
    assert!(
        section.contains("\"kind\": \"named\""),
        "missing named goroutine; section: {}",
        section
    );
    assert!(
        section.contains("\"kind\": \"anonymous\""),
        "missing anonymous goroutine; section: {}",
        section
    );
    // Total goroutine count = 3 (object boundaries are `}`).
    assert_eq!(
        section.matches("\"file\":").count(),
        3,
        "expected 3 goroutines in section; got: {}",
        section
    );
}

#[test]
fn detects_infinite_for_loops_as_long_running() {
    // worker.go has `for { select { ... } }` — the no-condition
    // for_statement shape that classifies as a long-running
    // run() body. main.go and handlers.go don't have one.
    let json = run_against_fixture();
    assert!(
        json.contains("\"long_loops\": [{\"file\": \"worker.go\"}]"),
        "expected one long-loop in worker.go; output:\n{}",
        json
    );
}

#[test]
fn handlers_in_main_are_not_misclassified_as_handlers() {
    // main.go has `func main()` and `func init()` — both should
    // be classified by name (main / init), NOT misclassified as
    // handlers despite main.go importing net/http. The handlers
    // section's main.go count should be 0.
    let json = run_against_fixture();
    let handlers_start = json.find("\"handlers\":").expect("handlers section");
    let handlers_end = json[handlers_start..]
        .find("],")
        .map(|e| handlers_start + e)
        .unwrap_or(json.len());
    let section = &json[handlers_start..handlers_end];
    assert!(
        !section.contains("main.go"),
        "main() / init() must not be classified as handlers; section: {}",
        section
    );
}
