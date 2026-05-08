//! `lotus` command-line entry point.
//!
//! For v0 (Phase 1 milestone 1), the CLI threads source through
//! lex → parse → AST and prints the result. No typecheck, no
//! codegen yet.
//!
//! Usage:
//!     lotus parse <file.lt>
//!     lotus lex   <file.lt>

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        usage();
        return ExitCode::from(2);
    }
    let cmd = &args[1];
    let path = PathBuf::from(&args[2]);

    let source = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("could not read {}: {}", path.display(), e);
            return ExitCode::from(1);
        }
    };

    match cmd.as_str() {
        "lex" => run_lex(&source),
        "parse" => run_parse(&source),
        other => {
            eprintln!("unknown command: {}", other);
            usage();
            ExitCode::from(2)
        }
    }
}

fn usage() {
    eprintln!("lotus — lotus language CLI (Phase 1 milestone 1)");
    eprintln!();
    eprintln!("Usage:");
    eprintln!("    lotus lex   <file.lt>     tokenize and print tokens");
    eprintln!("    lotus parse <file.lt>     parse and print the AST");
}

fn run_lex(source: &str) -> ExitCode {
    match lotus_syntax::lex(source) {
        Ok(tokens) => {
            for t in &tokens {
                let (line, col) = t.span.line_col(source);
                println!("{:>4}:{:<3} {:?}", line, col, t.kind);
            }
            ExitCode::SUCCESS
        }
        Err(diags) => {
            for d in &diags {
                eprintln!("{}", d.render(source));
            }
            ExitCode::from(1)
        }
    }
}

fn run_parse(source: &str) -> ExitCode {
    match lotus_syntax::parse_source(source) {
        Ok(prog) => {
            println!("{:#?}", prog);
            ExitCode::SUCCESS
        }
        Err(diags) => {
            for d in &diags {
                eprintln!("{}", d.render(source));
            }
            ExitCode::from(1)
        }
    }
}
