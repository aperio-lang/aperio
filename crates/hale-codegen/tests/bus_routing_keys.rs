//! Phase 3 routing-key end-to-end tests (2026-05-25). Drives
//! the full parser → typecheck → codegen → C-runtime pipeline
//! against small programs that exercise the swallow policy
//! (the v0.1 impl). See `spec/semantics.md` § "Phase 3:
//! routing keys" for the surface.

use std::process::Command;

use hale_codegen::build_executable;

fn build(name: &str, src: &str) -> std::path::PathBuf {
    let program = hale_syntax::parse_source(src).expect("parse");
    let mut bin = std::env::temp_dir();
    bin.push(format!("hale_test_bus_routing_keys_{}", name));
    build_executable(&program, &bin).expect("build");
    bin
}

/// Canonical multi-instance routing: two subscribers, each with
/// a different `where key == self.id` filter. The publisher
/// sends two messages with different `id` fields; each
/// subscriber receives ONLY the message whose key matches its
/// id, never both.
#[test]
fn keyed_subscribe_routes_to_matching_instance_only() {
    let src = r#"
        type Ev { id: Int; payload: Int; }
        topic K {
            payload: Ev;
            subject: "k";
            keyed_by id;
        }
        locus Sub {
            params { my_id: Int = 0; tag: String = "?"; }
            bus { subscribe K as on_k where key == self.my_id; }
            fn on_k(e: Ev) {
                println("sub.", self.tag, " got id=", e.id,
                        " payload=", e.payload);
            }
        }
        main locus App {
            params {
                a: Sub = Sub { my_id: 1, tag: "a" };
                b: Sub = Sub { my_id: 2, tag: "b" };
            }
            bus { publish K; }
            run() {
                K <- Ev { id: 1, payload: 100 };
                K <- Ev { id: 2, payload: 200 };
                K <- Ev { id: 1, payload: 101 };
            }
        }
        fn main() { App { }; }
    "#;
    let bin = build("multi_instance_routing", src);
    let out = Command::new(&bin).output().expect("run");
    let _ = std::fs::remove_file(&bin);
    assert!(out.status.success(), "non-zero exit: {:?}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    // sub.a (my_id=1) should see id=1 twice; sub.b (my_id=2)
    // should see id=2 once. Neither should see the OTHER key's
    // messages — that's the contract the routing-key primitive
    // is enforcing.
    let a_lines: Vec<&str> = stdout
        .lines()
        .filter(|ln| ln.starts_with("sub.a "))
        .collect();
    let b_lines: Vec<&str> = stdout
        .lines()
        .filter(|ln| ln.starts_with("sub.b "))
        .collect();
    assert_eq!(
        a_lines.len(),
        2,
        "expected 2 lines for sub.a (matched id=1 twice); got {:?}",
        a_lines
    );
    assert_eq!(
        b_lines.len(),
        1,
        "expected 1 line for sub.b (matched id=2 once); got {:?}",
        b_lines
    );
    // No cross-contamination.
    for ln in &a_lines {
        assert!(
            ln.contains("id=1"),
            "sub.a saw a non-1 id: {}",
            ln
        );
    }
    for ln in &b_lines {
        assert!(
            ln.contains("id=2"),
            "sub.b saw a non-2 id: {}",
            ln
        );
    }
}

/// Unmatched-key publishes drop silently (the v0.1 swallow
/// policy). Send a message whose key doesn't match any
/// subscriber — assert no handler fired.
#[test]
fn keyed_publish_swallows_when_no_subscriber_matches() {
    let src = r#"
        type Ev { id: Int; }
        topic K { payload: Ev; subject: "k"; keyed_by id; }
        locus Sub {
            params { my_id: Int = 0; }
            bus { subscribe K as on_k where key == self.my_id; }
            fn on_k(e: Ev) { println("got id=", e.id); }
        }
        main locus App {
            params { a: Sub = Sub { my_id: 1 }; }
            bus { publish K; }
            run() {
                K <- Ev { id: 999 };
                println("after publish");
            }
        }
        fn main() { App { }; }
    "#;
    let bin = build("swallow_no_match", src);
    let out = Command::new(&bin).output().expect("run");
    let _ = std::fs::remove_file(&bin);
    assert!(out.status.success(), "non-zero exit: {:?}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains("got id="),
        "expected no handler invocations (swallow); got: {:?}",
        stdout
    );
    assert!(stdout.contains("after publish"));
}

/// Decimal-typed routing keys (i128 routing space). Verifies
/// the high-half of the u128 carries through register + dispatch.
#[test]
fn keyed_subscribe_with_decimal_key() {
    let src = r#"
        type Ev { route: Decimal; v: Int; }
        topic K { payload: Ev; subject: "k"; keyed_by route; }
        locus Sub {
            params { r: Decimal = 0.0d; tag: String = "?"; }
            bus { subscribe K as on_k where key == self.r; }
            fn on_k(e: Ev) { println("sub.", self.tag, " v=", e.v); }
        }
        main locus App {
            params {
                a: Sub = Sub { r: 1.5d, tag: "a" };
                b: Sub = Sub { r: 2.5d, tag: "b" };
            }
            bus { publish K; }
            run() {
                K <- Ev { route: 1.5d, v: 10 };
                K <- Ev { route: 2.5d, v: 20 };
            }
        }
        fn main() { App { }; }
    "#;
    let bin = build("decimal_key", src);
    let out = Command::new(&bin).output().expect("run");
    let _ = std::fs::remove_file(&bin);
    assert!(out.status.success(), "non-zero exit: {:?}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let a_lines: Vec<&str> = stdout
        .lines()
        .filter(|ln| ln.starts_with("sub.a "))
        .collect();
    let b_lines: Vec<&str> = stdout
        .lines()
        .filter(|ln| ln.starts_with("sub.b "))
        .collect();
    assert_eq!(a_lines.len(), 1, "sub.a expected 1 line; got {:?}", a_lines);
    assert_eq!(b_lines.len(), 1, "sub.b expected 1 line; got {:?}", b_lines);
    assert!(a_lines[0].contains("v=10"), "got: {}", a_lines[0]);
    assert!(b_lines[0].contains("v=20"), "got: {}", b_lines[0]);
}

/// Unkeyed subscribers (no `where key ==`) on a KEYED topic
/// fire on EVERY keyed publish — they're the "audit-all sink"
/// pattern: a subscriber that wants to see all traffic on the
/// subject regardless of routing key. spec/semantics.md
/// § "Phase 3: routing keys" calls this out explicitly. Both
/// the specific-key sub and the unkeyed sub fire when key=1
/// matches the specific sub's filter.
#[test]
fn keyed_publish_fires_unkeyed_subscribers_as_audit_sinks() {
    let src = r#"
        type Ev { id: Int; }
        topic K { payload: Ev; subject: "k"; keyed_by id; }
        locus Specific {
            params { my_id: Int = 0; }
            bus { subscribe K as on_k where key == self.my_id; }
            fn on_k(e: Ev) { println("specific id=", e.id); }
        }
        locus Audit {
            bus { subscribe K as on_k; }
            fn on_k(e: Ev) { println("audit id=", e.id); }
        }
        main locus App {
            params {
                s: Specific = Specific { my_id: 1 };
                u: Audit = Audit { };
            }
            bus { publish K; }
            run() {
                K <- Ev { id: 1 };
                K <- Ev { id: 2 };
            }
        }
        fn main() { App { }; }
    "#;
    let bin = build("audit_sink_fires", src);
    let out = Command::new(&bin).output().expect("run");
    let _ = std::fs::remove_file(&bin);
    assert!(out.status.success(), "non-zero exit: {:?}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Specific (my_id=1) sees only id=1; audit sees both.
    let specific_lines: Vec<&str> = stdout
        .lines()
        .filter(|ln| ln.starts_with("specific "))
        .collect();
    let audit_lines: Vec<&str> = stdout
        .lines()
        .filter(|ln| ln.starts_with("audit "))
        .collect();
    assert_eq!(
        specific_lines.len(),
        1,
        "specific should fire once (key=1 match); got {:?}",
        specific_lines
    );
    assert!(
        specific_lines[0].contains("id=1"),
        "got: {}",
        specific_lines[0]
    );
    assert_eq!(
        audit_lines.len(),
        2,
        "audit-sink should fire twice (every keyed publish); got {:?}",
        audit_lines
    );
}

/// Backward-compat: unkeyed topics (no `keyed_by`) work exactly
/// as today. Unkeyed subscribers receive every publish. Lock-in
/// regression to make sure the codegen's keyed branch only
/// triggers when keyed_by is declared.
#[test]
fn unkeyed_topic_legacy_dispatch_unchanged() {
    let src = r#"
        type Ev { n: Int; }
        topic K { payload: Ev; subject: "k"; }
        locus A {
            bus { subscribe K as on_k; }
            fn on_k(e: Ev) { println("A n=", e.n); }
        }
        locus B {
            bus { subscribe K as on_k; }
            fn on_k(e: Ev) { println("B n=", e.n); }
        }
        main locus App {
            params {
                a: A = A { };
                b: B = B { };
            }
            bus { publish K; }
            run() { K <- Ev { n: 7 }; }
        }
        fn main() { App { }; }
    "#;
    let bin = build("unkeyed_legacy", src);
    let out = Command::new(&bin).output().expect("run");
    let _ = std::fs::remove_file(&bin);
    assert!(out.status.success(), "non-zero exit: {:?}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("A n=7"), "got: {:?}", stdout);
    assert!(stdout.contains("B n=7"), "got: {:?}", stdout);
}

/// Literal-key filter (no self.field involved). `where key == 1`
/// pins the subscriber to that specific value regardless of
/// instance state.
#[test]
fn keyed_subscribe_with_literal_key() {
    let src = r#"
        type Ev { id: Int; }
        topic K { payload: Ev; subject: "k"; keyed_by id; }
        locus Sub {
            params { tag: String = "?"; }
            bus { subscribe K as on_k where key == 42; }
            fn on_k(e: Ev) { println("sub.", self.tag, " id=", e.id); }
        }
        main locus App {
            params { s: Sub = Sub { tag: "a" }; }
            bus { publish K; }
            run() {
                K <- Ev { id: 1 };
                K <- Ev { id: 42 };
                K <- Ev { id: 100 };
            }
        }
        fn main() { App { }; }
    "#;
    let bin = build("literal_key", src);
    let out = Command::new(&bin).output().expect("run");
    let _ = std::fs::remove_file(&bin);
    assert!(out.status.success(), "non-zero exit: {:?}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = stdout
        .lines()
        .filter(|ln| ln.starts_with("sub.a "))
        .collect();
    assert_eq!(
        lines.len(),
        1,
        "literal-key sub fired wrong number of times: {:?}",
        lines
    );
    assert!(lines[0].contains("id=42"), "got: {}", lines[0]);
}
