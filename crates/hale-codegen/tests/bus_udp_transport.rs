//! 2026-05-26 — UDP bus transport (`udp://host:port`) end-to-end.
//! Single URL scheme covers both unicast and multicast: the
//! transport inspects the destination address and joins the
//! multicast group (IP_ADD_MEMBERSHIP) when it lands in
//! 224.0.0.0/4. Same `sendto` on the publisher side either way;
//! the kernel routes via the multicast tree for 224/4
//! destinations, regular path otherwise.
//!
//! Both tests spawn a subscriber binary (sleeps 1s while its
//! reader thread receives datagrams + dispatches them to the
//! local handler), then run a publisher binary that fires once
//! and exits. The subscriber's stdout is checked for the payload.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use hale_codegen::build_executable;

fn unique_path(tag: &str, ext: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut p = std::env::temp_dir();
    p.push(format!(
        "lt-bus-udp-{}-{}-{}.{}",
        tag,
        std::process::id(),
        nanos,
        ext,
    ));
    p
}

fn compile(tag: &str, src: &str) -> PathBuf {
    let program = hale_syntax::parse_source(src).expect("parse");
    let bin = unique_path(tag, "bin");
    build_executable(&program, &bin).expect("build");
    bin
}

fn subscriber_src() -> &'static str {
    r#"
        type Ping { n: Int; }
        locus Sub {
            bus {
                subscribe "evt" as on_evt of type Ping;
            }
            fn on_evt(p: Ping) {
                println("got n=", p.n);
            }
        }
        fn main() {
            Sub { };
            // Give the udp reader thread time to receive +
            // dispatch the publisher's datagram. The
            // cooperative scheduler drains the queue during
            // sleep ticks.
            std::time::sleep(800ms);
        }
    "#
}

fn publisher_src() -> &'static str {
    r#"
        type Ping { n: Int; }
        locus Pub {
            bus {
                publish "evt" of type Ping;
            }
            birth() {
                "evt" <- Ping { n: 4242 };
            }
        }
        fn main() {
            Pub { };
        }
    "#
}

fn run_pair(sub_bin: &PathBuf, pub_bin: &PathBuf,
            sub_cfg: &PathBuf, pub_cfg: &PathBuf) -> String
{
    // Subscriber spawned first so it binds before the publisher
    // sendto. Loopback delivery is essentially instant; the
    // 800ms sleep on the subscriber side covers the publisher
    // startup + sendto + reader-thread schedule latency.
    let sub = Command::new(sub_bin)
        .env("LOTUS_BUS_CONFIG", sub_cfg)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn subscriber");
    // Give the subscriber's reader thread ~100ms to bind the
    // socket + join the group (if multicast) before the
    // publisher sends.
    std::thread::sleep(Duration::from_millis(150));
    let pub_out = Command::new(pub_bin)
        .env("LOTUS_BUS_CONFIG", pub_cfg)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run publisher");
    assert!(
        pub_out.status.success(),
        "publisher exited non-zero: {:?}\nstderr: {}",
        pub_out.status,
        String::from_utf8_lossy(&pub_out.stderr),
    );
    let sub_out = sub.wait_with_output().expect("wait subscriber");
    assert!(
        sub_out.status.success(),
        "subscriber exited non-zero: {:?}\nstdout: {}\nstderr: {}",
        sub_out.status,
        String::from_utf8_lossy(&sub_out.stdout),
        String::from_utf8_lossy(&sub_out.stderr),
    );
    String::from_utf8_lossy(&sub_out.stdout).to_string()
}

#[test]
fn udp_unicast_delivers_payload_loopback() {
    let sub_bin = compile("uc_sub", subscriber_src());
    let pub_bin = compile("uc_pub", publisher_src());
    // Pick a port unlikely to collide; loopback so the actual
    // port doesn't matter much for routing.
    let port = 57781;
    let sub_cfg = unique_path("uc_sub", "conf");
    let pub_cfg = unique_path("uc_pub", "conf");
    std::fs::write(&sub_cfg, format!("evt = udp://127.0.0.1:{}:listen\n", port))
        .expect("write sub cfg");
    std::fs::write(&pub_cfg, format!("evt = udp://127.0.0.1:{}:connect\n", port))
        .expect("write pub cfg");

    let out = run_pair(&sub_bin, &pub_bin, &sub_cfg, &pub_cfg);

    let _ = std::fs::remove_file(&sub_bin);
    let _ = std::fs::remove_file(&pub_bin);
    let _ = std::fs::remove_file(&sub_cfg);
    let _ = std::fs::remove_file(&pub_cfg);

    assert!(
        out.contains("got n=4242"),
        "subscriber should receive the unicast datagram; \
         stdout:\n{}",
        out
    );
}

#[test]
fn udp_multicast_delivers_payload_loopback() {
    // Multicast group in the administratively-scoped block
    // (239.0.0.0/8) — guaranteed local-scope, won't route off-
    // host even on misconfigured networks. IP_MULTICAST_LOOP
    // defaults to 1 on Linux IPv4 so the sender receives its
    // own packets on loopback.
    let sub_bin = compile("mc_sub", subscriber_src());
    let pub_bin = compile("mc_pub", publisher_src());
    let group = "239.255.77.77";
    let port  = 57783;
    let sub_cfg = unique_path("mc_sub", "conf");
    let pub_cfg = unique_path("mc_pub", "conf");
    std::fs::write(&sub_cfg, format!("evt = udp://{}:{}:listen\n", group, port))
        .expect("write sub cfg");
    std::fs::write(&pub_cfg, format!("evt = udp://{}:{}:connect\n", group, port))
        .expect("write pub cfg");

    let out = run_pair(&sub_bin, &pub_bin, &sub_cfg, &pub_cfg);

    let _ = std::fs::remove_file(&sub_bin);
    let _ = std::fs::remove_file(&pub_bin);
    let _ = std::fs::remove_file(&sub_cfg);
    let _ = std::fs::remove_file(&pub_cfg);

    assert!(
        out.contains("got n=4242"),
        "subscriber should receive the multicast datagram \
         via the joined group; stdout:\n{}",
        out
    );
}
