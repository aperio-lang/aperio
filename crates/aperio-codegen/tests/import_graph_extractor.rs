//! m97: import-graph extractor end-to-end test.
//!
//! Builds `apps/import-graph/main.ap`, runs it against the
//! checked-in fixture (`apps/import-graph/fixture/`), and
//! asserts on the JSON tower output's shape and content. Light
//! parsing — uses substring checks rather than a full JSON
//! parser, since the v0 tower has a fixed shape and the goal is
//! to catch regressions in extraction logic, not validate JSON
//! conformance.

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
        .join("import-graph")
        .join("main.ap");
    let src = std::fs::read_to_string(&src_path).expect("read main.ap");
    let program = aperio_syntax::parse_source(&src).expect("parse main.ap");
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut bin = std::env::temp_dir();
    bin.push(format!(
        "aperio_import_graph_{}_{}",
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
        .join("import-graph")
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
fn tower_lists_every_go_file_under_main() {
    let json = run_against_fixture();
    // Every .go file in the fixture appears as a tower entry,
    // each labeled with the right package.
    for f in ["main.go", "greet.go", "server.go", "util.go"] {
        let needle = format!("\"id\": \"{}\"", f);
        assert!(
            json.contains(&needle),
            "expected entry for {}; output:\n{}",
            f,
            json
        );
    }
    // Package name is "main" for every file.
    let pkg_count = json.matches("\"package\": \"main\"").count();
    assert_eq!(
        pkg_count, 4,
        "expected 4 package=main entries; got {}; output:\n{}",
        pkg_count, json
    );
}

#[test]
fn tower_extracts_grouped_imports_correctly() {
    // greet.go uses the grouped import shape:
    //   import (
    //       "fmt"
    //       "strings"
    //   )
    // Extractor should flatten both into the imports array,
    // not stop at the first or skip them.
    let json = run_against_fixture();
    // Find greet.go's entry and assert on its imports.
    let needle = "\"id\": \"greet.go\"";
    let pos = json
        .find(needle)
        .unwrap_or_else(|| panic!("no greet.go entry; output:\n{}", json));
    // Slice from the entry to the next `}` to scope assertions.
    let entry = &json[pos..];
    let end = entry.find('}').unwrap_or(entry.len());
    let entry = &entry[..end];
    assert!(
        entry.contains("\"fmt\""),
        "expected greet.go to import fmt; entry: {:?}",
        entry
    );
    assert!(
        entry.contains("\"strings\""),
        "expected greet.go to import strings; entry: {:?}",
        entry
    );
}

#[test]
fn tower_extracts_single_imports_correctly() {
    // main.go has the single-import shape: `import "fmt"`. The
    // extractor's recursive walk under import_declaration should
    // pick it up regardless of whether import_spec is wrapped in
    // an import_spec_list or not.
    let json = run_against_fixture();
    let pos = json
        .find("\"id\": \"main.go\"")
        .unwrap_or_else(|| panic!("no main.go entry; output:\n{}", json));
    let entry = &json[pos..];
    let end = entry.find('}').unwrap_or(entry.len());
    let entry = &entry[..end];
    assert!(
        entry.contains("\"imports\": [\"fmt\"]"),
        "expected main.go imports=[\"fmt\"]; entry: {:?}",
        entry
    );
}

#[test]
fn tower_handles_file_with_no_imports() {
    // util.go has package_clause but no import_declaration. The
    // imports array should still be present, just empty.
    let json = run_against_fixture();
    let pos = json
        .find("\"id\": \"util.go\"")
        .unwrap_or_else(|| panic!("no util.go entry; output:\n{}", json));
    let entry = &json[pos..];
    let end = entry.find('}').unwrap_or(entry.len());
    let entry = &entry[..end];
    assert!(
        entry.contains("\"imports\": []"),
        "expected util.go imports=[]; entry: {:?}",
        entry
    );
}

#[test]
fn tower_preserves_slash_in_import_paths() {
    // server.go imports "net/http" — the / must survive the
    // quote-stripping pass without being treated as a path
    // separator anywhere.
    let json = run_against_fixture();
    assert!(
        json.contains("\"net/http\""),
        "expected net/http import preserved; output:\n{}",
        json
    );
}
