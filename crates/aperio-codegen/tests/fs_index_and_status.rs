//! Phase 2e + 2f — list_dir index API + read_file errno status.
//!
//! 2e closes `apps/ssg/FRICTION.md` 2026-05-10 list_dir-newline-string:
//! `list_dir(path) -> String` newline-joined still ships, but every
//! caller paid the cost of a manual `index_of("\n") + slice +
//! advance` loop. `list_dir_count` + `list_dir_at` route through
//! the same global-arena cache, so iteration becomes a clean
//! `while i < n` bounded by count.
//!
//! 2f closes `apps/ssg/FRICTION.md` 2026-05-10 read_file-empty-vs-error:
//! `read_file(path)` returns `""` for both "empty file" and "missing
//! file" because the C-layer `-1` clamps to `0` / empty-string at the
//! Aperio surface. `read_file_status(path)` returns 0 on success or
//! the platform errno on failure, so callers can disambiguate.

use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use aperio_codegen::build_executable;

fn build_and_run(name: &str, source: &str) -> (String, std::process::ExitStatus) {
    let program = aperio_syntax::parse_source(source).expect("parse");
    let mut bin = std::env::temp_dir();
    bin.push(format!("aperio_test_fsindex_{}", name));
    build_executable(&program, &bin).expect("build");
    let output = Command::new(&bin).output().expect("run");
    let _ = std::fs::remove_file(&bin);
    (String::from_utf8_lossy(&output.stdout).to_string(), output.status)
}

fn unique_dir(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut p = std::env::temp_dir();
    p.push(format!(
        "aperio_fsindex_{}_{}_{}.d",
        tag,
        std::process::id(),
        nanos
    ));
    std::fs::create_dir_all(&p).expect("mkdir");
    p
}

#[test]
fn list_dir_count_returns_entry_count() {
    let dir = unique_dir("count");
    for name in &["alpha.txt", "beta.md", "gamma.bin"] {
        std::fs::write(dir.join(name), b"x").expect("write");
    }
    let src = format!(
        r#"
        fn main() {{
            let n = std::io::fs::list_dir_count("{}");
            println("count=", n);
        }}
        "#,
        dir.display()
    );
    let (stdout, status) = build_and_run("count_three", &src);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(status.success(), "exit: {:?}", status);
    assert!(stdout.contains("count=3"), "got: {:?}", stdout);
}

#[test]
fn list_dir_at_walks_entries_in_order() {
    // The order of readdir entries isn't specified by POSIX, but
    // for any given directory the count + at pair must agree:
    // every index 0..count returns a valid entry name, and the
    // names collected by at(i) match what list_dir's newline form
    // produces.
    let dir = unique_dir("walk");
    for name in &["a.txt", "b.txt", "c.txt"] {
        std::fs::write(dir.join(name), b"y").expect("write");
    }
    let src = format!(
        r#"
        fn main() {{
            let p = "{}";
            let n = std::io::fs::list_dir_count(p);
            let mut i = 0;
            while i < n {{
                let name = std::io::fs::list_dir_at(p, i);
                println("e", i, "=", name);
                i = i + 1;
            }}
        }}
        "#,
        dir.display()
    );
    let (stdout, status) = build_and_run("walk_three", &src);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(status.success(), "exit: {:?}", status);
    // The test asserts presence, not order — readdir's order is
    // implementation-defined.
    assert!(stdout.contains("e0="), "expected e0= line; got: {:?}", stdout);
    assert!(stdout.contains("e1="), "got: {:?}", stdout);
    assert!(stdout.contains("e2="), "got: {:?}", stdout);
    for name in &["a.txt", "b.txt", "c.txt"] {
        assert!(
            stdout.contains(name),
            "missing {} in: {:?}",
            name,
            stdout
        );
    }
}

#[test]
fn list_dir_count_on_missing_dir_returns_zero() {
    let src = r#"
        fn main() {
            let n = std::io::fs::list_dir_count("/tmp/aperio_definitely_missing_xyz123_dir");
            println("count=", n);
        }
    "#;
    let (stdout, status) = build_and_run("missing", src);
    assert!(status.success(), "exit: {:?}", status);
    assert!(stdout.contains("count=0"), "got: {:?}", stdout);
}

#[test]
fn list_dir_at_out_of_range_returns_empty_string() {
    let dir = unique_dir("oob");
    std::fs::write(dir.join("only.txt"), b"z").expect("write");
    let src = format!(
        r#"
        fn main() {{
            let p = "{}";
            let n = std::io::fs::list_dir_count(p);
            println("n=", n);
            let valid = std::io::fs::list_dir_at(p, 0);
            println("valid_len=", len(valid));
            let oob = std::io::fs::list_dir_at(p, 5);
            println("oob_len=", len(oob));
            let neg = std::io::fs::list_dir_at(p, -1);
            println("neg_len=", len(neg));
        }}
        "#,
        dir.display()
    );
    let (stdout, status) = build_and_run("oob", &src);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(status.success(), "exit: {:?}", status);
    assert!(stdout.contains("n=1"), "got: {:?}", stdout);
    assert!(stdout.contains("valid_len=8"), "len('only.txt')=8; got: {:?}", stdout);
    assert!(stdout.contains("oob_len=0"), "got: {:?}", stdout);
    assert!(stdout.contains("neg_len=0"), "got: {:?}", stdout);
}

#[test]
fn read_file_status_zero_on_success() {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut path = std::env::temp_dir();
    path.push(format!("aperio_rfs_success_{}.txt", nanos));
    std::fs::write(&path, b"hello").expect("write");
    let src = format!(
        r#"
        fn main() {{
            let s = std::io::fs::read_file_status("{}");
            println("status=", s);
        }}
        "#,
        path.display()
    );
    let (stdout, status) = build_and_run("rfs_ok", &src);
    let _ = std::fs::remove_file(&path);
    assert!(status.success(), "exit: {:?}", status);
    assert!(stdout.contains("status=0"), "got: {:?}", stdout);
}

#[test]
fn read_file_status_enoent_on_missing_file() {
    // The classic friction: an empty-on-success read returns "" /
    // size=0; a missing-file read returns "" / size=-1 (clamped to
    // 0 / empty-string at the Aperio surface). Pre-2f, both
    // surface as `len(content) == 0` with no distinguisher.
    // read_file_status returns the errno (ENOENT == 2 on Linux,
    // ENOENT == 2 on macOS too) so callers can branch on it.
    let src = r#"
        fn main() {
            let content = std::io::fs::read_file("/tmp/aperio_rfs_missing_xyz123_file.txt");
            let status = std::io::fs::read_file_status("/tmp/aperio_rfs_missing_xyz123_file.txt");
            println("content_len=", len(content));
            println("status=", status);
        }
    "#;
    let (stdout, exit) = build_and_run("rfs_missing", src);
    assert!(exit.success(), "exit: {:?}", exit);
    assert!(stdout.contains("content_len=0"), "got: {:?}", stdout);
    // ENOENT is 2 on Linux/macOS — the canonical missing-file errno.
    assert!(stdout.contains("status=2"), "expected ENOENT=2; got: {:?}", stdout);
}

#[test]
fn read_file_status_zero_on_empty_file() {
    // Empty file = success with zero bytes. status=0 + content_len=0;
    // distinguishable from the missing-file case (status=2).
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut path = std::env::temp_dir();
    path.push(format!("aperio_rfs_empty_{}.txt", nanos));
    std::fs::write(&path, b"").expect("write empty");
    let src = format!(
        r#"
        fn main() {{
            let content = std::io::fs::read_file("{}");
            let status = std::io::fs::read_file_status("{}");
            println("content_len=", len(content));
            println("status=", status);
        }}
        "#,
        path.display(),
        path.display()
    );
    let (stdout, exit) = build_and_run("rfs_empty", &src);
    let _ = std::fs::remove_file(&path);
    assert!(exit.success(), "exit: {:?}", exit);
    assert!(stdout.contains("content_len=0"), "got: {:?}", stdout);
    assert!(stdout.contains("status=0"), "got: {:?}", stdout);
}
