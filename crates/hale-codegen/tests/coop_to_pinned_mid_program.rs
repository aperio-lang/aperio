//! the coop→pinned drain friction — coop→pinned mid-program dispatch.
//!
//! Pre-fix: a pinned locus's run() body that doesn't return
//! never drained its mailbox until dissolve. Cooperative
//! publishers could enqueue cells but the handler never fired
//! mid-program — the "imperative direct-method-call boot
//! loop" workaround was the only way to get bootstrap events
//! through.
//!
//! Post-fix: the pinned thread's `time::sleep` drains the
//! TLS-cached mailbox. A long-running pinned run() loop with
//! `sleep(N)` now picks up cells posted by cooperative
//! publishers within the next sleep window.

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
        "lt-coop-to-pinned-{}-{}-{}.bin",
        tag,
        std::process::id(),
        nanos,
    ));
    p
}

#[test]
fn pinned_run_loop_drains_mailbox_via_sleep() {
    // Bootstrapper (cooperative, default main pool) publishes
    // SubscribeRequest cells from its run(). Mdgw (pinned) is a
    // subscriber; its own run() spins in a sleep loop emulating
    // a forever-server shape. The handler bumps a global-ish
    // counter via println so we can observe whether cells fired
    // mid-program or only at dissolve.
    //
    // The "all 3 prints land before 'main done'" assertion is
    // the load-bearing property — without the fix the prints
    // emit at process-exit drain (after main done).
    let src = r#"
        type Req { n: Int; }

        locus Mdgw {
            bus { subscribe "sub" as on_sub of type Req; }
            fn on_sub(r: Req) {
                println("handled n=", r.n);
            }
            run() {
                // Forever-server shape: spin in sleep. The
                // mailbox-drain fold-in on sleep is what lets
                // on_sub fire here instead of at dissolve.
                let mut i = 0;
                while i < 10 {
                    std::time::sleep(10ms);
                    i = i + 1;
                }
            }
        }

        locus Boot {
            bus { publish "sub" of type Req; }
            run() {
                // Three publishes spaced so Mdgw's sleep loop
                // has a chance to wake between each. Total
                // sleep budget: ~30ms < Mdgw's ~100ms.
                std::time::sleep(5ms);
                "sub" <- Req { n: 1 };
                std::time::sleep(5ms);
                "sub" <- Req { n: 2 };
                std::time::sleep(5ms);
                "sub" <- Req { n: 3 };
            }
        }

        main locus App {
            params {
                mdgw: Mdgw = Mdgw { };
                boot: Boot = Boot { };
            }
            placement {
                mdgw: pinned;
            }
            run() {
                // Wait long enough that Mdgw's run() loop is
                // still spinning when we print 'main done'.
                // Without the mailbox-drain fold-in, handler
                // prints land AFTER 'main done'.
                std::time::sleep(80ms);
                println("main done");
                // Extra padding so the deferred-dissolve flush
                // doesn't catch the test in flight.
                std::time::sleep(50ms);
            }
        }

        fn main() {
            App { };
        }
    "#;

    let program = hale_syntax::parse_source(src).expect("parse");
    let bin = unique_path("mid");
    build_executable(&program, &bin).expect("build");
    let out = Command::new(&bin).output().expect("run");
    let _ = std::fs::remove_file(&bin);

    assert!(
        out.status.success(),
        "binary exited non-zero: {:?}\nstdout: {}\nstderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );

    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let lines: Vec<&str> = stdout.lines().collect();

    let main_done_idx = lines
        .iter()
        .position(|l| *l == "main done")
        .unwrap_or_else(|| {
            panic!(
                "expected 'main done' in stdout; got: {:?}",
                stdout
            );
        });
    let handled: Vec<(usize, &&str)> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.starts_with("handled n="))
        .collect();

    assert_eq!(
        handled.len(),
        3,
        "expected exactly 3 handler prints; got {}: {:?}",
        handled.len(),
        stdout
    );

    for (idx, line) in &handled {
        assert!(
            *idx < main_done_idx,
            "'{}' fired AFTER 'main done' (line {} > {}); \
             mailbox drained at dissolve, not mid-program. \
             Full stdout:\n{}",
            line,
            idx,
            main_done_idx,
            stdout
        );
    }
}
