//! F.32-1γ-v1 (2026-05-25) — `@form(hashmap, sync = lockfree,
//! cap = N)` end-to-end coverage.
//!
//! Three scenarios:
//!
//!   1. `lockfree_basic_single_pool` — sanity check that the
//!      lockfree opt-in compiles and the standard get/has/len
//!      surface works under no contention.
//!
//!   2. `lockfree_cross_pool_correctness` — the headline F.32-1γ
//!      scenario. Two pools (main + io) write disjoint keys into
//!      a single fixed-cap shared Registry; the test asserts
//!      both writers' inserts land (200k total entries) without
//!      memory corruption, livelock, or lost updates.
//!
//!   3. `lockfree_update_existing_key` — same-key write twice
//!      must update (not double-count). Verifies the CAS
//!      COMMITTED → CLAIMED → write → COMMITTED update path.

use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use hale_codegen::build_executable;

fn unique_path(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut p = std::env::temp_dir();
    p.push(format!(
        "lt-form-hashmap-lockfree-{}-{}-{}.bin",
        tag,
        std::process::id(),
        nanos,
    ));
    p
}

fn build_and_run(tag: &str, src: &str) -> (String, std::process::ExitStatus) {
    let program = hale_syntax::parse_source(src).expect("parse");
    let bin = unique_path(tag);
    build_executable(&program, &bin).expect("build");
    let out = Command::new(&bin).output().expect("run binary");
    let _ = std::fs::remove_file(&bin);
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    (stdout, out.status)
}

#[test]
fn lockfree_basic_single_pool() {
    let src = r#"
        type Counter { id: Int; v: Int; }

        @form(hashmap, sync = lockfree, cap = 2000)
        locus Registry {
            capacity { pool entries of Counter indexed_by id; }
        }

        main locus App {
            params { reg: Registry = Registry { }; }
            run() {
                let n = 1000;
                let mut i = 0;
                while i < n {
                    self.reg.set(Counter { id: i, v: i + 1 });
                    i = i + 1;
                }
                print("len="); println(self.reg.len());
                let e = self.reg.get(42) or raise;
                print("e.v="); println(e.v);
                let h = self.reg.has(99);
                print("has99="); println(h);
                let m = self.reg.has(99999);
                print("has99999="); println(m);
            }
        }

        fn main() { App { }; }
    "#;
    let (stdout, status) = build_and_run("basic", src);
    assert!(
        status.success(),
        "binary exited non-zero: {:?}\nstdout: {}",
        status,
        stdout,
    );
    assert!(stdout.contains("len=1000"), "got: {:?}", stdout);
    assert!(stdout.contains("e.v=43"), "got: {:?}", stdout);
    assert!(stdout.contains("has99=true"), "got: {:?}", stdout);
    assert!(stdout.contains("has99999=false"), "got: {:?}", stdout);
}

#[test]
fn lockfree_cross_pool_correctness() {
    // The scenario γ-v1 was sized for. Two pools concurrently
    // write disjoint keys (even / odd ids); the CAS-based slot
    // claim must let both pools race on different cells without
    // losing entries.
    let src = r#"
        type Counter { id: Int; v: Int; }

        @form(hashmap, sync = lockfree, cap = 60000)
        locus Registry {
            capacity { pool entries of Counter indexed_by id; }
        }

        locus PoolHost {
            params { reg: Registry = Registry { }; }
            run() {
                let mut i = 0;
                while i < 20000 {
                    self.reg.set(Counter { id: i * 2 + 1, v: i });
                    i = i + 1;
                }
            }
        }

        main locus App {
            params { host: PoolHost = PoolHost { }; }
            placement { host: cooperative(pool = io); }
            run() {
                let mut i = 0;
                while i < 20000 {
                    self.host.reg.set(Counter { id: i * 2, v: i });
                    i = i + 1;
                }
                while self.host.reg.len() < 40000 {
                    std::time::sleep(1ms);
                }
                print("len="); println(self.host.reg.len());
            }
        }

        fn main() { App { }; }
    "#;
    let (stdout, status) = build_and_run("cross_pool", src);
    assert!(
        status.success(),
        "binary exited non-zero (probable CAS or memory-order bug): {:?}\nstdout: {}",
        status,
        stdout,
    );
    assert!(
        stdout.contains("len=40000"),
        "expected len=40000 after both writers drain; got:\n{}",
        stdout
    );
}

#[test]
fn lockfree_update_existing_key() {
    // Update path: same key written twice. Verifies the CAS
    // COMMITTED → CLAIMED → write-value → COMMITTED transition
    // works (γ-v1's update branch in set_lockfree).
    let src = r#"
        type Counter { id: Int; v: Int; }

        @form(hashmap, sync = lockfree, cap = 64)
        locus Registry {
            capacity { pool entries of Counter indexed_by id; }
        }

        main locus App {
            params { reg: Registry = Registry { }; }
            run() {
                self.reg.set(Counter { id: 7, v: 100 });
                self.reg.set(Counter { id: 7, v: 200 });   // update
                print("len="); println(self.reg.len());
                let e = self.reg.get(7) or raise;
                print("v="); println(e.v);
            }
        }

        fn main() { App { }; }
    "#;
    let (stdout, status) = build_and_run("update", src);
    assert!(
        status.success(),
        "binary exited non-zero: {:?}\nstdout: {}",
        status,
        stdout,
    );
    assert!(stdout.contains("len=1"), "update should not double-count; got: {:?}", stdout);
    assert!(stdout.contains("v=200"), "update should overwrite; got: {:?}", stdout);
}
