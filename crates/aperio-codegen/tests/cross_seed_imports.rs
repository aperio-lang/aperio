//! v1.x-IMPORT PR3: end-to-end test for cross-seed imports.
//!
//! Builds the `import-toy-consumer` fixture against the `lib-toy`
//! vendored library. Verifies:
//!
//! 1. The consumer's `import "lib-toy" as toy;` resolves to the
//!    sibling directory; both `greet.ap` and `format.ap` are
//!    parsed and merged into the binary.
//! 2. The mangler's unified rename map across the lib lets
//!    `greet.ap` reference `Formatted` (declared in `format.ap`)
//!    correctly — the cross-file intra-seed reference rewrites
//!    to the same mangled name that `format.ap`'s decl ends up at.
//! 3. The consumer's `toy::Greeter { ... }` literal and
//!    `toy::Formatted` references resolve through the per-build
//!    path-rename table that `build_executable_with_imports`
//!    consults.
//!
//! The test replicates the CLI's resolve-and-mangle flow inline
//! (rather than spawning the `aperio` binary) so it stays
//! hermetic relative to whatever's in `target/debug/`. The CLI's
//! own file-resolution glue is verified manually during dev.

use std::path::PathBuf;
use std::process::Command;

use aperio_codegen::build_executable_with_imports;
use aperio_codegen::mangle;
use aperio_syntax::ast::{Program, TopDecl};
use aperio_syntax::parse_source;

fn fixtures_dir() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests");
    p.push("fixtures");
    p
}

fn top_name(d: &TopDecl) -> Option<&str> {
    match d {
        TopDecl::Locus(l) => Some(&l.name.name),
        TopDecl::Perspective(p) => Some(&p.name.name),
        TopDecl::Type(t) => Some(&t.name.name),
        TopDecl::Const(c) => Some(&c.name.name),
        TopDecl::Fn(f) => Some(&f.name.name),
        TopDecl::Interface(i) => Some(&i.name.name),
        TopDecl::Topic(t) => Some(&t.name.name),
        TopDecl::Module(_) => None,
    }
}

/// Replicate the CLI's resolve-and-mangle pipeline for one
/// import: read every `.ap` in the lib directory, parse, build
/// a unified rename map across files, mangle each, and return
/// (merged_lib_items, per-build_renames).
fn resolve_and_mangle_lib(
    lib_dir: &PathBuf,
    alias: &str,
) -> (Vec<TopDecl>, Vec<(Vec<String>, String)>) {
    let mut files: Vec<PathBuf> = std::fs::read_dir(lib_dir)
        .expect("read lib dir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("ap"))
        .collect();
    files.sort();
    let mut parsed: Vec<(String, Program)> = Vec::new();
    for f in &files {
        let src = std::fs::read_to_string(f).expect("read lib file");
        let prog = parse_source(&src).expect("parse lib file");
        let stem = f.file_stem().and_then(|s| s.to_str()).unwrap_or("x").to_string();
        parsed.push((stem, prog));
    }
    // Build the unified rename map and apply it.
    let stem_refs: Vec<(String, &Program)> =
        parsed.iter().map(|(s, p)| (s.clone(), p)).collect();
    let seed_renames = mangle::build_seed_renames(&stem_refs, alias);
    let mut renames: Vec<(Vec<String>, String)> = Vec::new();
    for (name, mangled) in &seed_renames {
        renames.push((vec![alias.to_string(), name.clone()], mangled.clone()));
    }
    let mut items: Vec<TopDecl> = Vec::new();
    for (_, mut prog) in parsed {
        mangle::mangle_with_renames(&mut prog, &seed_renames);
        items.extend(prog.items);
    }
    (items, renames)
}

#[test]
fn or_on_path_callee_for_imported_fallible_fn() {
    // #66 regression: `alias::fn(args) or raise` and
    // `alias::fn(args) or fallback` codegen against a path
    // callee. Before the fix the Path callee shape rejected
    // with "or callee shape not yet supported: Discriminant(2)".
    let lib_dir = fixtures_dir().join("lib-fallible");
    let consumer_src_path = fixtures_dir()
        .join("import-fallible-consumer")
        .join("main.ap");

    let consumer_src = std::fs::read_to_string(&consumer_src_path)
        .expect("read consumer main.ap");
    let mut consumer_prog =
        parse_source(&consumer_src).expect("parse consumer");
    consumer_prog.imports.clear();

    let (lib_items, renames) = resolve_and_mangle_lib(&lib_dir, "lp");
    consumer_prog.items.extend(lib_items);

    let mut bin = std::env::temp_dir();
    bin.push(format!(
        "aperio_or_on_path_callee_{}",
        std::process::id()
    ));
    build_executable_with_imports(&consumer_prog, &bin, &renames)
        .expect("build consumer + lib");

    let out = Command::new(&bin).output().expect("run consumer");
    let _ = std::fs::remove_file(&bin);
    assert!(
        out.status.success(),
        "non-zero exit: {:?} stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("a=4"), "got: {:?}", stdout);
    assert!(stdout.contains("b=99"), "got: {:?}", stdout);
}

#[test]
fn consumer_uses_greeter_and_formatted_from_lib_toy() {
    let lib_dir = fixtures_dir().join("lib-toy");
    let consumer_src_path = fixtures_dir()
        .join("import-toy-consumer")
        .join("main.ap");

    let consumer_src = std::fs::read_to_string(&consumer_src_path)
        .expect("read consumer main.ap");
    let mut consumer_prog =
        parse_source(&consumer_src).expect("parse consumer");

    // The consumer's import line is just metadata for the CLI's
    // file-resolution glue; the codegen sees a merged program
    // with the imports already resolved + mangled. Drop the
    // imports list so codegen doesn't try to follow them on its
    // own (it doesn't, but defensive).
    consumer_prog.imports.clear();

    let (lib_items, renames) = resolve_and_mangle_lib(&lib_dir, "toy");
    // Sanity: the lib produced both decls under the toy alias.
    let lib_names: Vec<String> = lib_items
        .iter()
        .filter_map(top_name)
        .map(|s| s.to_string())
        .collect();
    assert!(
        lib_names.contains(&"__lib_toy_greet_Greeter".to_string()),
        "Greeter not in mangled lib: {:?}",
        lib_names
    );
    assert!(
        lib_names.contains(&"__lib_toy_format_Formatted".to_string()),
        "Formatted not in mangled lib: {:?}",
        lib_names
    );

    consumer_prog.items.extend(lib_items);

    let mut bin = std::env::temp_dir();
    bin.push(format!(
        "aperio_cross_seed_imports_{}",
        std::process::id()
    ));
    build_executable_with_imports(&consumer_prog, &bin, &renames)
        .expect("build consumer + lib");

    let out = Command::new(&bin).output().expect("run consumer binary");
    let _ = std::fs::remove_file(&bin);
    assert!(
        out.status.success(),
        "non-zero exit: {:?} stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("hi"),
        "expected Greeter prefix in stdout: {:?}",
        stdout
    );
    assert!(
        stdout.contains("world"),
        "expected Formatted body in stdout: {:?}",
        stdout
    );
}
