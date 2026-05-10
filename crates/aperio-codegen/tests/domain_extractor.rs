//! m102 v0: domain extractor end-to-end test.
//!
//! Builds `apps/domain-graph/main.ap`, runs it against the
//! checked-in fixture (`apps/domain-graph/fixture/`), and asserts
//! on per-type motion-form rewrites. The morpheme rewriter is
//! the load-bearing piece: lookup table hits, suffix rule with
//! min-stem guard, CamelCase split, and explicit `<unknown:X>`
//! markers must each behave per the design doc.

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

fn run_against_fixture() -> String {
    let src_path = workspace_root()
        .join("apps")
        .join("domain-graph")
        .join("main.ap");
    let src = std::fs::read_to_string(&src_path).expect("read main.ap");
    let program = aperio_syntax::parse_source(&src).expect("parse main.ap");
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut bin = std::env::temp_dir();
    bin.push(format!(
        "aperio_domain_{}_{}",
        std::process::id(),
        nanos
    ));
    build_executable(&program, &bin).expect("build extractor");
    let fixture = workspace_root()
        .join("apps")
        .join("domain-graph")
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
fn lookup_table_hits_produce_canonical_motion_forms() {
    // Controller, Repository, Cache are in the seed lookup; their
    // motion forms come straight from the table.
    let json = run_against_fixture();
    assert!(
        json.contains("\"name\": \"Controller\", \"motion\": \"controlling\""),
        "missing Controller→controlling; output:\n{}",
        json
    );
    assert!(
        json.contains("\"name\": \"Repository\", \"motion\": \"carrying\""),
        "missing Repository→carrying; output:\n{}",
        json
    );
    assert!(
        json.contains("\"name\": \"Cache\", \"motion\": \"remembering\""),
        "missing Cache→remembering; output:\n{}",
        json
    );
}

#[test]
fn suffix_rule_handles_long_enough_morphemes() {
    // Greeter is 7 chars, passes the min-6 guard; -er strips →
    // "Greet" → lowercase → "greet" + "ing" → "greeting".
    let json = run_against_fixture();
    assert!(
        json.contains("\"name\": \"Greeter\", \"motion\": \"greeting\""),
        "missing Greeter→greeting (suffix rule); output:\n{}",
        json
    );
}

#[test]
fn camel_case_compounds_split_and_rewrite_per_morpheme() {
    // OrderProcessor: "Order" (5 chars, fails min-6 → unknown)
    // + "Processor" (lookup hit → processing). The honest
    // motion form preserves Order's uncertainty rather than
    // fabricating "ording".
    let json = run_against_fixture();
    assert!(
        json.contains(
            "\"name\": \"OrderProcessor\", \"motion\": \"<unknown:Order>-processing\""
        ),
        "expected honest CamelCase rewrite; output:\n{}",
        json
    );
    // UserValidator: "User" (4 chars, unknown) + "Validator"
    // (lookup → checking).
    assert!(
        json.contains(
            "\"name\": \"UserValidator\", \"motion\": \"<unknown:User>-checking\""
        ),
        "expected honest UserValidator rewrite; output:\n{}",
        json
    );
}

#[test]
fn unknown_morphemes_surface_as_explicit_markers() {
    // Foobaz: not in lookup, no -er/-or/-ar suffix → marked
    // unknown verbatim. **Honesty about uncertainty** — design
    // doc constraint, non-negotiable at v0.
    let json = run_against_fixture();
    assert!(
        json.contains("\"name\": \"Foobaz\", \"motion\": \"<unknown:Foobaz>\""),
        "expected Foobaz→<unknown:Foobaz>; output:\n{}",
        json
    );
}

#[test]
fn no_motion_form_is_silently_fabricated() {
    // Negative assertion: nothing in the output should claim a
    // motion form for "Order" or "User" alone (since both fall
    // below the suffix rule's min-stem threshold). Catches a
    // regression where someone weakens the threshold.
    let json = run_against_fixture();
    assert!(
        !json.contains("\"motion\": \"ording\""),
        "regression: Order→ording fabrication detected; output:\n{}",
        json
    );
    assert!(
        !json.contains("\"motion\": \"using\""),
        "regression: User→using fabrication detected; output:\n{}",
        json
    );
}
