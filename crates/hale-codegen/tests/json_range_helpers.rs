//! 2026-05-26 — range-bearing JSON iter_find variants +
//! std::str::range_* helpers. The fathom team identified
//! `iter_find_string_field` returning an owned String per
//! field lookup as the dominant arena-pressure source on
//! large JSON-walk workloads (a 5 MB Coinbase level2 frame
//! with 100k+ elements). The range variants return (start,
//! end_exclusive) byte positions inside the source json
//! String instead — paired with std::str::range_eq /
//! range_parse_int / range_parse_decimal, the full walk
//! runs allocation-free.
//!
//! Tests exercise the headline shape: walk an order-book
//! snapshot, compare a string field to a literal, parse
//! a Decimal field. Plus the missing-field and malformed-
//! input paths.
//!
//! Earlier zero-element-copy work (json_span_iter.rs) cut
//! per-iter cost from O(element_size) to O(value_size) by
//! avoiding the per-element substring copy. These tests
//! complete the picture by avoiding the per-VALUE substring
//! copy too.

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
        "lt-json-range-{}-{}-{}.bin",
        tag,
        std::process::id(),
        nanos,
    ));
    p
}

fn build_and_run(name: &str, src: &str) -> (String, std::process::ExitStatus) {
    let program = hale_syntax::parse_source(src).expect("parse");
    let bin = unique_path(name);
    build_executable(&program, &bin).expect("build");
    let out = Command::new(&bin).output().expect("run");
    let _ = std::fs::remove_file(&bin);
    (
        String::from_utf8_lossy(&out.stdout).to_string(),
        out.status,
    )
}

#[test]
fn range_eq_matches_substring() {
    // Sanity: std::str::range_eq compares (json, start, end)
    // against an expected literal, byte-for-byte.
    let src = r#"
        fn main() {
            let s = "hello world";
            let h = std::str::range_eq(s, 0, 5, "hello");
            let w = std::str::range_eq(s, 6, 11, "world");
            let m = std::str::range_eq(s, 0, 5, "world");
            let l = std::str::range_eq(s, 0, 4, "hello");  // length mismatch
            println("h=", h, " w=", w, " m=", m, " l=", l);
        }
    "#;
    let (stdout, status) = build_and_run("range_eq", src);
    assert!(status.success(), "non-zero exit: {:?}\nstdout: {}", status, stdout);
    assert!(stdout.contains("h=true"), "got: {:?}", stdout);
    assert!(stdout.contains("w=true"), "got: {:?}", stdout);
    assert!(stdout.contains("m=false"), "byte mismatch must report false; got: {:?}", stdout);
    assert!(stdout.contains("l=false"), "length mismatch must report false; got: {:?}", stdout);
}

#[test]
fn range_parse_int_strict() {
    let src = r#"
        fn main() {
            let s = "[42][-7][bad]";
            let a = std::str::range_parse_int(s, 1, 3) or raise;
            let b = std::str::range_parse_int(s, 5, 7) or raise;
            println("a=", a, " b=", b);
            // Malformed sub-range reports ParseError.
            let _c = std::str::range_parse_int(s, 9, 12) or fallback();
        }

        fn fallback() -> Int { println("caught_parse_error"); return -1; }
    "#;
    let (stdout, status) = build_and_run("range_parse_int", src);
    assert!(status.success(), "non-zero exit: {:?}\nstdout: {}", status, stdout);
    assert!(stdout.contains("a=42"), "got: {:?}", stdout);
    assert!(stdout.contains("b=-7"), "got: {:?}", stdout);
    assert!(stdout.contains("caught_parse_error"), "malformed input must surface ParseError; got: {:?}", stdout);
}

#[test]
fn range_parse_decimal_strict() {
    let src = r#"
        fn main() {
            let s = "[100.5][nope][-0.001]";
            let a = std::str::range_parse_decimal(s, 1, 6) or raise;
            let c = std::str::range_parse_decimal(s, 14, 20) or raise;
            println("a=", a);
            println("c=", c);
            let _b = std::str::range_parse_decimal(s, 8, 12) or fallback();
        }

        fn fallback() -> Decimal { println("caught_parse_error"); return 0.0d; }
    "#;
    let (stdout, status) = build_and_run("range_parse_decimal", src);
    assert!(status.success(), "non-zero exit: {:?}\nstdout: {}", status, stdout);
    assert!(stdout.contains("a=100.5"), "got: {:?}", stdout);
    assert!(stdout.contains("c=-0.001"), "got: {:?}", stdout);
    assert!(stdout.contains("caught_parse_error"), "malformed input must surface ParseError; got: {:?}", stdout);
}

#[test]
fn iter_find_field_range_walks_array() {
    // The fathom headline shape: walk an L2 snapshot array,
    // compare side, parse price + size as Decimal. Whole loop
    // runs allocation-free per element (after the source body
    // is in arena).
    let src = r#"
        fn main() {
            let body = "[{\"side\":\"bid\",\"price\":\"100.5\",\"size\":\"1.25\"},{\"side\":\"offer\",\"price\":\"200\",\"size\":\"0.5\"}]";
            let mut it = std::json::array_first_span(body);
            let mut bid_count = 0;
            let mut ask_count = 0;
            let mut total_size = 0.0d;
            while !it.done {
                let side_r = std::json::iter_find_string_field_range(it, body, "side");
                if std::str::range_eq(body, side_r.start, side_r.end_pos, "bid") {
                    bid_count = bid_count + 1;
                } else if std::str::range_eq(body, side_r.start, side_r.end_pos, "offer") {
                    ask_count = ask_count + 1;
                }
                let size_r = std::json::iter_find_string_field_range(it, body, "size");
                let sz = std::str::range_parse_decimal(
                    body, size_r.start, size_r.end_pos
                ) or raise;
                total_size = total_size + sz;
                it = std::json::array_next_span(it, body);
            }
            println("bids=", bid_count, " asks=", ask_count);
            println("total_size=", total_size);
        }
    "#;
    let (stdout, status) = build_and_run("walk", src);
    assert!(status.success(), "non-zero exit: {:?}\nstdout: {}", status, stdout);
    assert!(stdout.contains("bids=1"), "got: {:?}", stdout);
    assert!(stdout.contains("asks=1"), "got: {:?}", stdout);
    // 1.25 + 0.5 = 1.75
    assert!(stdout.contains("total_size=1.75"), "got: {:?}", stdout);
}

#[test]
fn iter_find_field_range_missing_field_reports_not_ok() {
    let src = r#"
        fn main() {
            let body = "[{\"side\":\"bid\"},{\"side\":\"offer\"}]";
            let mut it = std::json::array_first_span(body);
            while !it.done {
                let price_r = std::json::iter_find_field_range(it, body, "price");
                if price_r.ok {
                    println("found_price");
                } else {
                    println("missing_price");
                }
                it = std::json::array_next_span(it, body);
            }
        }
    "#;
    let (stdout, status) = build_and_run("missing", src);
    assert!(status.success(), "non-zero exit: {:?}\nstdout: {}", status, stdout);
    // Both elements lack "price"; expect 2 missing_price lines.
    let n = stdout.matches("missing_price").count();
    assert_eq!(n, 2, "expected 2 missing_price; got: {:?}", stdout);
    assert!(!stdout.contains("found_price"), "no element has the field; got: {:?}", stdout);
}
