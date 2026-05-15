//! v1.x-VIOLATE (F.27) — codegen / compiled-binary tests for
//! `violate NAME;`. Exercises the lowering end-to-end: compile
//! to a native binary, run it, verify stdout.

use std::process::Command;

use aperio_codegen::build_executable;

fn build_and_run(name: &str, source: &str) -> (String, std::process::ExitStatus) {
    let program = aperio_syntax::parse_source(source).expect("parse");
    let mut bin = std::env::temp_dir();
    bin.push(format!("lotus_test_{}", name));
    build_executable(&program, &bin).expect("build");
    let output = Command::new(&bin).output().expect("run");
    let _ = std::fs::remove_file(&bin);
    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        output.status,
    )
}

#[test]
fn violate_routes_to_parent_on_failure_in_native_binary() {
    let src = r#"
locus Child {
    closure fatal_io { epoch inline; }
    fn step() {
        violate fatal_io;
    }
}

locus Parent {
    accept(c: Child) { }
    on_failure(c: Child, err: ClosureViolation) {
        println("absorbed closure=", err.closure);
    }
    run() {
        let c = Child { };
        c.step();
        println("parent.run continued");
    }
}

fn main() { Parent { }; }
"#;
    let (stdout, status) = build_and_run("violate_routes", src);
    assert!(status.success(), "non-zero: {:?}", status);
    assert!(
        stdout.contains("absorbed closure=fatal_io"),
        "expected absorbed closure name in stdout; got: {:?}",
        stdout
    );
    assert!(
        stdout.contains("parent.run continued"),
        "expected run() to keep going after Child.step diverged; got: {:?}",
        stdout
    );
}

#[test]
fn self_draining_reads_true_after_violate_in_compiled() {
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
            println("ok draining");
        } else {
            println("FAIL not draining");
        }
    }
}

fn main() { Parent { }; }
"#;
    let (stdout, status) = build_and_run("violate_draining", src);
    assert!(status.success());
    assert!(
        stdout.contains("ok draining"),
        "expected draining flag set; got: {:?}",
        stdout
    );
}

#[test]
fn statement_after_violate_does_not_execute_in_compiled() {
    let src = r#"
locus Child {
    params { reached: Int = 0; }
    closure fatal { epoch inline; }
    fn step() {
        violate fatal;
        self.reached = 1;
    }
    fn check() -> Int { return self.reached; }
}

locus Parent {
    accept(c: Child) { }
    on_failure(c: Child, err: ClosureViolation) { }
    run() {
        let c = Child { };
        c.step();
        if c.check() == 0 {
            println("ok tail unreached");
        } else {
            println("FAIL tail ran");
        }
    }
}

fn main() { Parent { }; }
"#;
    let (stdout, status) = build_and_run("violate_divergent", src);
    assert!(status.success());
    assert!(
        stdout.contains("ok tail unreached"),
        "expected stmt after violate to be skipped; got: {:?}",
        stdout
    );
}
