//! Regression test for the Decimal-field-in-struct segfault
//! (fathom F7-segfault, 2026-05-20). Root cause:
//! `lotus_arena_alloc` was aligning the offset within the chunk
//! rather than the actual returned pointer address. The chunk's
//! data region starts after a 24-byte header — 8-byte aligned
//! but NOT 16-byte aligned — so allocating a struct with i128
//! fields (Decimal) landed at an 8-byte address. LLVM's `movaps`
//! store of i128 into struct fields then trapped with SIGSEGV.
//!
//! Fix in two layers:
//!   - codegen `arena_alloc` now passes align=16 (covers the
//!     widest scalar — i128) instead of the previous 8.
//!   - C `lotus_arena_alloc` computes the offset as a function
//!     of the actual returned pointer address, not just the
//!     within-chunk offset. The chunk's `(chunk+1) + used` cursor
//!     gets aligned to `align`, then converted back to an offset.

use std::process::Command;

use aperio_codegen::build_executable;

fn build_and_run(name: &str, source: &str) -> std::process::Output {
    let program = aperio_syntax::parse_source(source).expect("parse");
    let mut bin = std::env::temp_dir();
    bin.push(format!("aperio_test_dec_align_{}", name));
    build_executable(&program, &bin).expect("build");
    let out = Command::new(&bin).output().expect("run");
    let _ = std::fs::remove_file(&bin);
    out
}

#[test]
fn struct_with_decimal_default_does_not_segfault() {
    // The minimal repro. Pre-fix: SIGSEGV on the i128 store at
    // construction. Post-fix: exits cleanly.
    let src = r#"
        type X { p: Decimal = 0.0d; }
        fn main() {
            let x = X { };
            println("p=", x.p);
        }
    "#;
    let out = build_and_run("single_field", src);
    assert!(
        out.status.success(),
        "expected clean exit; status={:?} stderr={:?}",
        out.status,
        String::from_utf8_lossy(&out.stderr),
    );
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("p=0"),
        "got: {:?}",
        String::from_utf8_lossy(&out.stdout),
    );
}

#[test]
fn struct_with_many_decimal_fields_does_not_segfault() {
    // fathom's actual shape — a struct with two Decimal fields,
    // used as the default-init type on a high-field-count locus
    // (SymbolBook with 20 BookLevel fields).
    let src = r#"
        type BookLevel {
            price: Decimal = 0.0d;
            qty: Decimal = 0.0d;
        }
        locus SymbolBook {
            params {
                b1:  BookLevel = BookLevel { };
                b2:  BookLevel = BookLevel { };
                b3:  BookLevel = BookLevel { };
                b4:  BookLevel = BookLevel { };
                b5:  BookLevel = BookLevel { };
                b6:  BookLevel = BookLevel { };
                b7:  BookLevel = BookLevel { };
                b8:  BookLevel = BookLevel { };
                b9:  BookLevel = BookLevel { };
                b10: BookLevel = BookLevel { };
                a1:  BookLevel = BookLevel { };
                a2:  BookLevel = BookLevel { };
                a3:  BookLevel = BookLevel { };
                a4:  BookLevel = BookLevel { };
                a5:  BookLevel = BookLevel { };
                a6:  BookLevel = BookLevel { };
                a7:  BookLevel = BookLevel { };
                a8:  BookLevel = BookLevel { };
                a9:  BookLevel = BookLevel { };
                a10: BookLevel = BookLevel { };
            }
        }
        fn main() {
            let sb = SymbolBook { };
            println("ok");
        }
    "#;
    let out = build_and_run("symbol_book_20", src);
    assert!(
        out.status.success(),
        "expected clean exit; status={:?} stderr={:?}",
        out.status,
        String::from_utf8_lossy(&out.stderr),
    );
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("ok"),
        "got: {:?}",
        String::from_utf8_lossy(&out.stdout),
    );
}

#[test]
fn multi_decimal_fallible_fn_returning_struct_does_not_segfault() {
    // fathom F4: multi-Decimal flat-struct return from a fallible
    // free-fn into a local binding. The friction noted F4
    // didn't repro in a smoke test, only in mdgw's runtime path.
    // After the alignment fix this shape works end-to-end —
    // F7-segfault and F4 shared the same root cause.
    let src = r#"
        type L1Update {
            symbol: String = "";
            bid: Decimal = 0.0d;
            bid_qty: Decimal = 0.0d;
            ask: Decimal = 0.0d;
            ask_qty: Decimal = 0.0d;
        }
        fn parse_ticker(s: String) -> L1Update fallible(ParseError) {
            return L1Update {
                symbol: s,
                bid: 100.5d,
                bid_qty: 1.0d,
                ask: 101.5d,
                ask_qty: 2.0d,
            };
        }
        fn main() {
            let l1 = parse_ticker("XBT/USD") or L1Update { };
            println("symbol=", l1.symbol);
            println("bid=", l1.bid);
            println("ask=", l1.ask);
        }
    "#;
    let out = build_and_run("parse_ticker", src);
    assert!(
        out.status.success(),
        "expected clean exit; status={:?} stderr={:?}",
        out.status,
        String::from_utf8_lossy(&out.stderr),
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("symbol=XBT/USD"), "got: {:?}", stdout);
    assert!(stdout.contains("bid=100.5"), "got: {:?}", stdout);
    assert!(stdout.contains("ask=101.5"), "got: {:?}", stdout);
}

#[test]
fn locus_with_decimal_param_default_does_not_segfault() {
    // Same shape via the locus-param-default path.
    let src = r#"
        locus Tracker {
            params {
                threshold: Decimal = 0.001d;
                total: Decimal = 0.0d;
            }
            fn show() {
                println("t=", self.threshold);
                println("s=", self.total);
            }
        }
        fn main() {
            let t = Tracker { };
            t.show();
        }
    "#;
    let out = build_and_run("locus_decimal_params", src);
    assert!(
        out.status.success(),
        "expected clean exit; status={:?} stderr={:?}",
        out.status,
        String::from_utf8_lossy(&out.stderr),
    );
}
