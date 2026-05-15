//! v1.x-VIOLATE (F.27) — typecheck rules for inline closures
//! and the `violate` statement.

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
fn canonical_shape_typechecks_clean() {
    // The error-check-fn pattern from F.27 / styleguide pattern 7.
    let src = r#"
locus L {
    params { last_error: String = ""; }
    closure fatal_io { captures: last_error; epoch inline; }
    fn handle(detail: String) -> Int {
        self.last_error = detail;
        violate fatal_io;
        return 0;
    }
}
fn main() { L { }; }
"#;
    let msgs = check(src);
    assert!(
        msgs.iter().all(|m| !m.contains("violate") && !m.contains("captures")),
        "expected clean typecheck, got: {:?}",
        msgs
    );
}

#[test]
fn self_draining_resolves_as_bool() {
    let src = r#"
locus L {
    closure fatal { epoch inline; }
    fn step() {
        if !self.draining {
            let _ = 0;
        }
    }
}
fn main() { L { }; }
"#;
    let msgs = check(src);
    assert!(
        msgs.iter().all(|m| !m.contains("draining") && !m.contains("no field")),
        "expected self.draining to typecheck, got: {:?}",
        msgs
    );
}

#[test]
fn violate_in_free_fn_rejected() {
    let src = r#"
fn helper() {
    violate fatal;
}
fn main() { }
"#;
    let msgs = check(src);
    assert!(
        msgs.iter().any(|m| m.contains("free fns can't use `violate`")),
        "expected free-fn rejection, got: {:?}",
        msgs
    );
}

#[test]
fn violate_in_on_failure_rejected() {
    let src = r#"
locus Child { }
locus Parent {
    closure fatal { epoch inline; }
    accept(c: Child) { }
    on_failure(c: Child, err: ClosureViolation) {
        violate fatal;
    }
}
fn main() { Parent { }; }
"#;
    let msgs = check(src);
    assert!(
        msgs.iter().any(|m| m.contains("not allowed inside an `on_failure` body")),
        "expected on_failure rejection, got: {:?}",
        msgs
    );
}

#[test]
fn violate_unknown_closure_rejected() {
    let src = r#"
locus L {
    closure fatal { epoch inline; }
    fn step() {
        violate ghost;
    }
}
fn main() { L { }; }
"#;
    let msgs = check(src);
    assert!(
        msgs.iter().any(|m| m.contains("has no closure named `ghost`")),
        "expected unknown-closure rejection, got: {:?}",
        msgs
    );
}

#[test]
fn violate_non_inline_closure_rejected() {
    let src = r#"
locus L {
    params { x: Int = 0; }
    closure check { self.x ~~ self.x within 0; epoch tick; }
    fn step() {
        violate check;
    }
}
fn main() { L { }; }
"#;
    let msgs = check(src);
    assert!(
        msgs.iter().any(|m| m.contains("is not declared `epoch inline`")),
        "expected non-inline rejection, got: {:?}",
        msgs
    );
}

#[test]
fn inline_closure_with_assertion_rejected() {
    let src = r#"
locus L {
    params { x: Int = 0; }
    closure bad { self.x ~~ self.x within 0; epoch inline; }
}
fn main() { L { }; }
"#;
    let msgs = check(src);
    assert!(
        msgs.iter().any(|m| m.contains("must omit the assertion")),
        "expected inline-with-assertion rejection, got: {:?}",
        msgs
    );
}

#[test]
fn captures_on_non_inline_closure_rejected() {
    let src = r#"
locus L {
    params { x: Int = 0; }
    closure check {
        self.x ~~ self.x within 0;
        captures: x;
        epoch tick;
    }
}
fn main() { L { }; }
"#;
    let msgs = check(src);
    assert!(
        msgs.iter().any(|m| {
            m.contains("`captures:` is meaningful only on `epoch inline` closures")
        }),
        "expected captures-non-inline rejection, got: {:?}",
        msgs
    );
}

#[test]
fn captures_missing_field_rejected() {
    let src = r#"
locus L {
    params { x: Int = 0; }
    closure fatal { captures: ghost; epoch inline; }
}
fn main() { L { }; }
"#;
    let msgs = check(src);
    assert!(
        msgs.iter().any(|m| m.contains("references field `ghost`")),
        "expected missing-field rejection, got: {:?}",
        msgs
    );
}

#[test]
fn assertion_less_non_inline_rejected() {
    let src = r#"
locus L {
    closure stub { }
}
fn main() { L { }; }
"#;
    let msgs = check(src);
    assert!(
        msgs.iter().any(|m| m.contains("missing assertion")),
        "expected missing-assertion rejection, got: {:?}",
        msgs
    );
}

#[test]
fn violate_with_payload_typechecks() {
    let src = r#"
locus L {
    closure fatal { epoch inline; }
    fn step() {
        violate fatal with 42;
    }
}
fn main() { L { }; }
"#;
    let msgs = check(src);
    assert!(
        msgs.iter().all(|m| !m.contains("violate")),
        "expected violate-with-payload to typecheck clean, got: {:?}",
        msgs
    );
}
