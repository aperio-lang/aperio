//! F.31 (2026-05-23) — placement block typecheck rules.
//!
//! The parser already enforces "main-only" and Ident keying.
//! Typecheck-side validation adds:
//!   1. Field exists in this locus's params block.
//!   2. Field type is a locus type.
//!   3. No duplicate field keys across placement entries.
//! Pinned-class restrictions (no accept(), no closures on
//! placed-pinned loci) move to codegen-time in Phase 3.

use aperio_syntax::parse_source;
use aperio_types::check_program;

fn check(src: &str) -> Vec<String> {
    let prog = parse_source(src).expect("parse failed");
    check_program(&prog)
        .into_iter()
        .map(|d| d.message)
        .collect()
}

#[test]
fn canonical_placement_typechecks_clean() {
    let src = r#"
locus Worker { run() { } }

main locus App {
    params {
        w: Worker = Worker { };
    }
    placement {
        w: pinned;
    }
}

fn main() { App { }; }
"#;
    let msgs = check(src);
    assert!(
        msgs.iter().all(|m| !m.contains("placement")),
        "expected clean placement typecheck, got: {:?}",
        msgs
    );
}

#[test]
fn placement_with_unknown_field_rejected() {
    let src = r#"
locus Worker { run() { } }

main locus App {
    params {
        w: Worker = Worker { };
    }
    placement {
        missing: pinned;
    }
}

fn main() { App { }; }
"#;
    let msgs = check(src);
    assert!(
        msgs.iter().any(|m|
            m.contains("placement") && m.contains("missing")
            && m.contains("params")),
        "expected diagnostic about unknown field `missing`, got: {:?}",
        msgs
    );
}

#[test]
fn placement_on_non_locus_field_rejected() {
    let src = r#"
main locus App {
    params {
        n: Int = 0;
    }
    placement {
        n: pinned;
    }
}

fn main() { App { }; }
"#;
    let msgs = check(src);
    assert!(
        msgs.iter().any(|m|
            m.contains("placement") && m.contains("not a locus type")),
        "expected diagnostic about non-locus type, got: {:?}",
        msgs
    );
}

#[test]
fn placement_duplicate_field_rejected() {
    let src = r#"
locus Worker { run() { } }

main locus App {
    params {
        w: Worker = Worker { };
    }
    placement {
        w: pinned;
        w: cooperative;
    }
}

fn main() { App { }; }
"#;
    let msgs = check(src);
    assert!(
        msgs.iter().any(|m|
            m.contains("duplicate") && m.contains("w")),
        "expected diagnostic about duplicate field, got: {:?}",
        msgs
    );
}

#[test]
fn placement_two_siblings_distinct_placements_clean() {
    let src = r#"
locus Worker { run() { } }

main locus App {
    params {
        a: Worker = Worker { };
        b: Worker = Worker { };
    }
    placement {
        a: pinned(core = 1);
        b: pinned(core = 2);
    }
}

fn main() { App { }; }
"#;
    let msgs = check(src);
    assert!(
        msgs.iter().all(|m| !m.contains("placement")),
        "expected two-sibling placement to typecheck clean, got: {:?}",
        msgs
    );
}

#[test]
fn placement_cooperative_with_pool_clean() {
    let src = r#"
locus Worker { run() { } }

main locus App {
    params {
        w: Worker = Worker { };
    }
    placement {
        w: cooperative(pool = io);
    }
}

fn main() { App { }; }
"#;
    let msgs = check(src);
    assert!(
        msgs.iter().all(|m| !m.contains("placement")),
        "expected cooperative-with-pool placement to typecheck clean, got: {:?}",
        msgs
    );
}

#[test]
fn placement_unspecified_field_uses_default() {
    // A locus without a placement entry doesn't need one; it
    // defaults to cooperative(pool = main) at codegen time.
    // Typecheck should not require placement coverage.
    let src = r#"
locus Worker { run() { } }

main locus App {
    params {
        a: Worker = Worker { };
        b: Worker = Worker { };
    }
    placement {
        a: pinned;
        // b deliberately not mentioned
    }
}

fn main() { App { }; }
"#;
    let msgs = check(src);
    assert!(
        msgs.iter().all(|m| !m.contains("placement")),
        "expected partial placement coverage to typecheck clean, got: {:?}",
        msgs
    );
}
