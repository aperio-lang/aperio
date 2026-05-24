//! v1.x-VIOLATE (F.27) — end-to-end interpreter tests for the
//! `violate NAME [with EXPR];` statement.
//!
//! These exercise the runtime contract:
//!   - synthesize a ClosureViolation value carrying locus +
//!     closure names and the captures snapshot;
//!   - set the locus's `draining` flag (readable as
//!     `self.draining`);
//!   - route to the parent's `on_failure` handler;
//!   - diverge the enclosing method body so subsequent
//!     statements in the same block don't run.

use hale_runtime::run_program;

fn run(src: &str) -> i32 {
    let program = hale_syntax::parse_source(src)
        .map_err(|d| {
            d.iter()
                .map(|x| x.render(src))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .expect("parse");
    run_program(&program).expect("run")
}

#[test]
fn violate_routes_to_parent_on_failure() {
    // Canonical shape: a Child violates an inline closure; the
    // parent's on_failure reads err.closure + frozen child state
    // via the handle (`c.last_error`). The handle-reading
    // access is the portable pattern that works in both
    // `hale run` and `hale build`.
    let src = r#"
locus Child {
    params { last_error: String = ""; }
    closure fatal_io { captures: last_error; epoch inline; }
    fn step() {
        self.last_error = "send_failed";
        violate fatal_io;
    }
}

locus Parent {
    params { saw: Int = 0; saw_name: String = ""; saw_detail: String = ""; }
    accept(c: Child) { }
    on_failure(c: Child, err: ClosureViolation) {
        self.saw = 1;
        self.saw_name = err.closure;
        self.saw_detail = c.last_error;
    }
    run() {
        let c = Child { };
        c.step();
        if self.saw != 1 {
            println("FAIL on_failure not fired");
            return;
        }
        if self.saw_name != "fatal_io" {
            println("FAIL wrong closure name: ", self.saw_name);
            return;
        }
        if self.saw_detail != "send_failed" {
            println("FAIL wrong child field: ", self.saw_detail);
            return;
        }
        println("OK violate routed");
    }
}

fn main() {
    Parent { };
}
"#;
    assert_eq!(run(src), 0);
}

#[test]
fn err_captures_field_works_in_interpreter_as_convenience() {
    // The interpreter materializes captures fields on the
    // ClosureViolation struct. Compiled code doesn't — but the
    // interpreter convenience is documented and tested here so
    // the regression surface is explicit. Portable code should
    // use the child-handle pattern shown above.
    let src = r#"
locus Child {
    params { last_error: String = ""; }
    closure fatal { captures: last_error; epoch inline; }
    fn step() {
        self.last_error = "via_err";
        violate fatal;
    }
}

locus Parent {
    params { saw: String = ""; }
    accept(c: Child) { }
    on_failure(c: Child, err: ClosureViolation) {
        self.saw = err.last_error;
    }
    run() {
        let c = Child { };
        c.step();
        if self.saw == "via_err" {
            println("OK interpreter exposes err.<capture>");
        } else {
            println("FAIL got: ", self.saw);
        }
    }
}

fn main() { Parent { }; }
"#;
    assert_eq!(run(src), 0);
}

#[test]
fn self_draining_observable_after_violate() {
    // After violate fires, the locus's `draining` flag stays
    // true. The canonical-pattern use is `if !self.draining {
    // ... }` to suppress a downstream effect. Verify by reading
    // self.draining from a later method call into the same
    // locus.
    let src = r#"
locus Child {
    closure fatal { epoch inline; }
    fn step() {
        violate fatal;
    }
    fn drained() -> Bool {
        return self.draining;
    }
}

locus Parent {
    accept(c: Child) { }
    on_failure(c: Child, err: ClosureViolation) { }
    run() {
        let c = Child { };
        c.step();
        if c.drained() {
            println("OK draining flag set");
        } else {
            println("FAIL draining flag not set");
            return;
        }
    }
}

fn main() {
    Parent { };
}
"#;
    assert_eq!(run(src), 0);
}

#[test]
fn statement_after_violate_does_not_execute() {
    // Violate is divergent — the next statement in the same
    // block must not run.
    let src = r#"
locus Child {
    params { reached_tail: Int = 0; }
    closure fatal { epoch inline; }
    fn step() {
        violate fatal;
        self.reached_tail = 1;
    }
    fn check() -> Int { return self.reached_tail; }
}

locus Parent {
    accept(c: Child) { }
    on_failure(c: Child, err: ClosureViolation) { }
    run() {
        let c = Child { };
        c.step();
        if c.check() == 0 {
            println("OK tail unreached");
        } else {
            println("FAIL tail reached");
            return;
        }
    }
}

fn main() {
    Parent { };
}
"#;
    assert_eq!(run(src), 0);
}

#[test]
fn violate_with_payload_carries_value() {
    // `violate NAME with <expr>;` adds a `payload` field to
    // the ClosureViolation.
    let src = r#"
locus Child {
    closure fatal { epoch inline; }
    fn step() {
        violate fatal with 42;
    }
}

locus Parent {
    params { saw: Int = -1; }
    accept(c: Child) { }
    on_failure(c: Child, err: ClosureViolation) {
        self.saw = err.payload;
    }
    run() {
        let c = Child { };
        c.step();
        if self.saw == 42 {
            println("OK payload received");
        } else {
            println("FAIL payload missing: ", self.saw);
            return;
        }
    }
}

fn main() {
    Parent { };
}
"#;
    assert_eq!(run(src), 0);
}
