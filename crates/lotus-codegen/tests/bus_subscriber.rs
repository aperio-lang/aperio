//! m59: subscriber-side reader thread integration test.
//!
//! Builds two lotus binaries — a subscriber and a publisher —
//! and exercises the full cross-process bus path end-to-end:
//!
//!   subscriber (LISTEN role)               publisher (CONNECT role)
//!     |                                       |
//!     | spawn reader thread (blocks accept)   | <- "evt" | Ping{n}
//!     |   <----------- AF_UNIX SEQPACKET -----|
//!     | recv → lotus_bus_local_dispatch       |
//!     | enqueue cell on cooperative queue     |
//!     | main: time::sleep(500ms); yield;      |
//!     | drain → Sub.on_evt(Ping{n})           |
//!     |   prints "subscriber got n=..."       |
//!
//! The subscriber's stdout is asserted to contain the printed
//! Ping value, proving the reader thread → local dispatch path
//! delivers cross-process publishes into a real lotus handler.
//!
//! At v0.1 the wire format is raw struct bytes: same arch +
//! same compiler version means identical layout on both sides.
//! The serializer milestone (m60+) replaces this with a
//! field-by-field little-endian encoding to defend against
//! padding drift across binary versions and to enable
//! heterogeneous-host targets.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use lotus_codegen::build_executable;

fn unique_path(tag: &str, ext: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut p = std::env::temp_dir();
    p.push(format!(
        "lt-m59-{}-{}-{}.{}",
        tag,
        std::process::id(),
        nanos,
        ext,
    ));
    p
}

fn build_binary(src: &str, tag: &str) -> PathBuf {
    let program = lotus_syntax::parse_source(src).expect("parse");
    let bin = unique_path(tag, "bin");
    build_executable(&program, &bin).expect("build");
    bin
}

#[test]
fn two_lotus_binaries_round_trip_a_publish() {
    // Sentinel chosen so each byte is unique + ASCII-printable —
    // shows up as "HGFEDCBA" in stdout if a layout regression
    // ever scrambles the bytes.
    let sentinel: i64 = 0x4142_4344_4546_4748;

    let subscriber_src = r#"
        type Ping {
            n: Int;
        }

        locus Sub {
            bus {
                subscribe "evt" as on_evt of type Ping;
            }
            fn on_evt(p: Ping) {
                println("subscriber got n=", p.n);
            }
        }

        fn main() {
            Sub { };
            // Wait for the cross-process publish to arrive. The
            // reader thread (spawned by lotus_bus_load_config when
            // the LISTEN-role config entry registers) enqueues the
            // cell on the cooperative queue while we're blocked in
            // sleep. After sleep, `yield` drains the queue ->
            // fires Sub.on_evt synchronously on the main thread.
            time::sleep(500ms);
            yield;
        }
    "#;

    let publisher_src = format!(
        r#"
        type Ping {{
            n: Int;
        }}

        // Dummy local subscriber so the BusState gate is satisfied
        // at compile time (codegen errors on `<-` without any
        // `bus subscribe` declared somewhere in the program).
        locus Sub {{
            bus {{
                subscribe "evt" as on_evt of type Ping;
            }}
            fn on_evt(p: Ping) {{
                println("publisher local got n=", p.n);
            }}
        }}

        locus Pub {{
            bus {{
                publish "evt" of type Ping;
            }}
            birth() {{
                "evt" <- Ping {{ n: {} }};
            }}
        }}

        fn main() {{
            Sub {{ }};
            Pub {{ }};
        }}
    "#,
        sentinel,
    );

    let sub_bin = build_binary(subscriber_src, "sub");
    let pub_bin = build_binary(&publisher_src, "pub");

    let sock = unique_path("sock", "sock");
    let sub_cfg = unique_path("subcfg", "conf");
    let pub_cfg = unique_path("pubcfg", "conf");
    std::fs::write(
        &sub_cfg,
        format!("evt = unix://{} : listen\n", sock.display()),
    )
    .expect("write sub cfg");
    std::fs::write(
        &pub_cfg,
        format!("evt = unix://{} : connect\n", sock.display()),
    )
    .expect("write pub cfg");

    // Spawn the subscriber first so its reader thread gets to
    // bind/listen before the publisher's connect-with-retry
    // starts. The connect side retries on ENOENT for ~1s, but
    // listener-first is the natural order and keeps the test
    // deterministic.
    let subscriber = Command::new(&sub_bin)
        .env("LOTUS_BUS_CONFIG", &sub_cfg)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn subscriber");

    // Brief delay so the subscriber's reader thread has a chance
    // to call lotus_transport_create(LISTEN) -> bind/listen
    // before the publisher tries to connect. Not strictly
    // required (connect retries) but reduces stderr noise from
    // ENOENT-and-backoff messages.
    std::thread::sleep(Duration::from_millis(50));

    let pub_out = Command::new(&pub_bin)
        .env("LOTUS_BUS_CONFIG", &pub_cfg)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run publisher");

    let sub_out = subscriber.wait_with_output().expect("subscriber wait");

    let _ = std::fs::remove_file(&sock);
    let _ = std::fs::remove_file(&sub_cfg);
    let _ = std::fs::remove_file(&pub_cfg);
    let _ = std::fs::remove_file(&sub_bin);
    let _ = std::fs::remove_file(&pub_bin);

    assert!(
        pub_out.status.success(),
        "publisher exited non-zero: {:?}\nstdout: {}\nstderr: {}",
        pub_out.status,
        String::from_utf8_lossy(&pub_out.stdout),
        String::from_utf8_lossy(&pub_out.stderr),
    );
    assert!(
        sub_out.status.success(),
        "subscriber exited non-zero: {:?}\nstdout: {}\nstderr: {}",
        sub_out.status,
        String::from_utf8_lossy(&sub_out.stdout),
        String::from_utf8_lossy(&sub_out.stderr),
    );

    let sub_stdout = String::from_utf8_lossy(&sub_out.stdout);
    let expected_line = format!("subscriber got n={}", sentinel);
    assert!(
        sub_stdout.contains(&expected_line),
        "subscriber stdout should contain '{}'; got: {:?}\n\
         publisher stderr: {}",
        expected_line,
        sub_stdout,
        String::from_utf8_lossy(&pub_out.stderr),
    );
}
