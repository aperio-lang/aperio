//! m73a: stdlib loci via bundled-source concatenation.
//!
//! Verifies that `std::io::tcp::Listener { ... }` (a path-
//! qualified struct literal referencing a stdlib locus) parses,
//! lowers, and runs end-to-end. The bundled stdlib source
//! (`runtime/stdlib.ap`) declares `__StdIoTcpListener`; codegen
//! prepends those decls to the user program before lowering and
//! rewrites the path-qualified instantiation to the mangled name.
//!
//! m73a ships a placeholder lifecycle (println-only). m73b
//! replaces birth/run/dissolve bodies with real lotus_tcp_*
//! calls via low-level `std::io::tcp::__*` path-call primitives.

use std::process::Command;

use aperio_codegen::build_executable;

fn build_and_run(name: &str, source: &str) -> (String, std::process::ExitStatus) {
    let program = aperio_syntax::parse_source(source).expect("parse");
    let mut bin = std::env::temp_dir();
    bin.push(format!("aperio_test_stdlib_locus_{}", name));
    build_executable(&program, &bin).expect("build");
    let output = Command::new(&bin).output().expect("run");
    let _ = std::fs::remove_file(&bin);
    (String::from_utf8_lossy(&output.stdout).to_string(), output.status)
}

#[test]
fn stdlib_locus_path_resolves_to_bundled_decl() {
    // The user references `std::io::tcp::Listener` by qualified
    // path; the codegen rewrites it to `__StdIoTcpListener`
    // (declared in runtime/stdlib.ap) and runs that locus's
    // lifecycle methods. m73a lifecycle bodies just println so
    // we can verify the resolution chain end-to-end without
    // actually opening sockets.
    let src = r#"
        fn main() {
            std::io::tcp::Listener { host: "127.0.0.1", port: 9001 };
        }
    "#;
    let (stdout, status) = build_and_run("path_resolves", src);
    assert!(status.success(), "non-zero: {:?}", status);
    assert!(
        stdout.contains("__StdIoTcpListener.birth host=127.0.0.1 port=9001"),
        "birth() didn't fire as expected; got: {:?}",
        stdout
    );
    assert!(
        stdout.contains("__StdIoTcpListener.run host=127.0.0.1 port=9001"),
        "run() didn't fire as expected; got: {:?}",
        stdout
    );
    assert!(
        stdout.contains("__StdIoTcpListener.dissolve host=127.0.0.1 port=9001"),
        "dissolve() didn't fire as expected; got: {:?}",
        stdout
    );
}

#[test]
fn stdlib_locus_uses_default_params_when_omitted() {
    // The bundled stdlib source declares `host: String =
    // "127.0.0.1"` and `port: Int = 0` defaults — instantiation
    // can omit either. Confirms the standard locus-default
    // mechanism flows through unchanged for stdlib loci.
    let src = r#"
        fn main() {
            std::io::tcp::Listener { port: 8080 };
        }
    "#;
    let (stdout, status) = build_and_run("defaults", src);
    assert!(status.success());
    assert!(
        stdout.contains("host=127.0.0.1 port=8080"),
        "expected default host with overridden port; got: {:?}",
        stdout
    );
}

#[test]
fn unknown_stdlib_path_struct_literal_errors_clearly() {
    // A `std::*` path that has no entry in STDLIB_PATH_RENAMES
    // must surface a clean diagnostic — same shape as a typo on
    // a user-declared locus name.
    let src = r#"
        fn main() {
            std::io::tcp::Nonexistent { port: 1 };
        }
    "#;
    let program = aperio_syntax::parse_source(src).expect("parse");
    let mut bin = std::env::temp_dir();
    bin.push("aperio_test_stdlib_locus_unknown");
    let result = build_executable(&program, &bin);
    let _ = std::fs::remove_file(&bin);
    assert!(result.is_err(), "expected build error for unknown stdlib path");
    let msg = format!("{:?}", result.unwrap_err());
    assert!(
        msg.contains("std::io::tcp::Nonexistent"),
        "diagnostic should name the unresolved path; got: {}",
        msg
    );
}

#[test]
fn user_program_with_no_stdlib_use_still_compiles() {
    // Concatenating stdlib decls onto every user program must
    // not break programs that don't reference std::*. Locks in
    // that the bundled `__StdIoTcpListener` doesn't pollute the
    // user namespace or interfere with the existing main()
    // discovery.
    let src = r#"
        fn main() {
            println("hello, world");
        }
    "#;
    let (stdout, status) = build_and_run("no_stdlib_use", src);
    assert!(status.success());
    assert!(stdout.contains("hello, world"));
    // The bundled stdlib locus has no fn main and is never
    // instantiated, so its placeholder println output must not
    // leak into a program that doesn't use it.
    assert!(
        !stdout.contains("__StdIoTcpListener"),
        "stdlib output leaked into program that didn't use stdlib; got: {:?}",
        stdout
    );
}
