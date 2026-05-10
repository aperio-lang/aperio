//! m76: Phase 1 capstone — integration test for examples/io-demo.
//!
//! Builds the example, runs it twice (default-config and
//! seeded-config paths) sequentially within a single test
//! function. Sequencing matters because the example hardcodes
//! port 9876 and shared /tmp paths — running two cases in
//! parallel would race both. Cargo's default per-test
//! parallelism would split the cases across threads, so we
//! keep them in one test.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use aperio_codegen::build_executable;

fn examples_dir() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.pop();
    p.push("examples");
    p
}

fn cleanup_demo_paths() {
    let _ = std::fs::remove_file("/tmp/aperio_io_demo_config.txt");
    let _ = std::fs::remove_file("/tmp/aperio_io_demo_log.txt");
}

fn wait_until_listening(port: u16) -> bool {
    for _ in 0..50 {
        if let Ok(s) = std::net::TcpStream::connect(("127.0.0.1", port)) {
            drop(s);
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    false
}

fn run_io_demo() -> (String, String, std::process::ExitStatus) {
    let mut src_path = examples_dir();
    src_path.push("io-demo");
    src_path.push("main.ap");
    let source = std::fs::read_to_string(&src_path).expect("read source");
    let program = aperio_syntax::parse_source(&source).expect("parse");

    let mut bin = std::env::temp_dir();
    bin.push(format!("aperio_io_demo_{}", std::process::id()));
    build_executable(&program, &bin).expect("build");

    let listener_proc = Command::new(&bin)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn io-demo");

    assert!(
        wait_until_listening(9876),
        "io-demo never bound to 127.0.0.1:9876"
    );

    let mut sock = std::net::TcpStream::connect(("127.0.0.1", 9876))
        .expect("connect to demo");
    let _ = sock.write_all(b"hello\n");
    drop(sock);

    let out = listener_proc.wait_with_output().expect("listener wait");
    let _ = std::fs::remove_file(&bin);

    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    (stdout, stderr, out.status)
}

#[test]
fn io_demo_capstone_default_then_with_config() {
    // ---- Cycle 1: no config file present, default payload ----
    cleanup_demo_paths();
    let (stdout, stderr, status) = run_io_demo();

    assert!(
        status.success(),
        "io-demo exited non-zero: {:?}\nstderr: {}",
        status,
        stderr
    );
    assert!(
        stdout.contains("config: none, using default"),
        "default-cycle: expected default-config message; got: {:?}",
        stdout
    );
    assert!(
        stdout.contains("io-demo: listening on 127.0.0.1:9876"),
        "default-cycle: expected listening diagnostic; got: {:?}",
        stdout
    );
    assert!(
        stdout.contains("io-demo: wrote log to /tmp/aperio_io_demo_log.txt"),
        "default-cycle: expected log-write success; got: {:?}",
        stdout
    );

    let log = std::fs::read_to_string("/tmp/aperio_io_demo_log.txt")
        .expect("default-cycle: log should exist");
    assert_eq!(
        log, "default visit\n",
        "default-cycle: expected default payload; got: {:?}",
        log
    );

    // ---- Cycle 2: seed the config and re-run ----
    cleanup_demo_paths();
    std::fs::write(
        "/tmp/aperio_io_demo_config.txt",
        "configured visit payload\n",
    )
    .expect("seed config");

    let (stdout, stderr, status) = run_io_demo();

    assert!(
        status.success(),
        "with-config cycle exited non-zero: {:?}\nstderr: {}",
        status,
        stderr
    );
    assert!(
        stdout.contains("config: loaded from /tmp/aperio_io_demo_config.txt"),
        "with-config cycle: expected loaded-config message; got: {:?}",
        stdout
    );

    let log = std::fs::read_to_string("/tmp/aperio_io_demo_log.txt")
        .expect("with-config cycle: log should exist");
    assert_eq!(
        log, "configured visit payload\n",
        "with-config cycle: expected config payload; got: {:?}",
        log
    );

    cleanup_demo_paths();
}
