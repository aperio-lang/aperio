//! Builtin functions in scope without `import`.
//!
//! v0 cut: `print`, `println`, `time::sleep`. Future cuts add
//! the rest of the stdlib (collections, strings, math) — these
//! three are the minimum to make the example ladder run.

use std::rc::Rc;
use std::thread;
use std::time::Duration;

use crate::value::{BuiltinRef, Value};

pub fn install_builtins(env: &crate::env::Env) {
    env.define(
        "print",
        Value::Builtin(BuiltinRef {
            name: "print",
            func: Rc::new(builtin_print),
        }),
    );
    env.define(
        "println",
        Value::Builtin(BuiltinRef {
            name: "println",
            func: Rc::new(builtin_println),
        }),
    );
}

fn builtin_print(args: &[Value]) -> Result<Value, String> {
    use std::io::Write;
    let body: String = args.iter().map(|v| v.display()).collect();
    let mut out = std::io::stdout().lock();
    out.write_all(body.as_bytes()).map_err(|e| e.to_string())?;
    out.flush().map_err(|e| e.to_string())?;
    Ok(Value::Unit)
}

fn builtin_println(args: &[Value]) -> Result<Value, String> {
    let body: String = args.iter().map(|v| v.display()).collect();
    println!("{}", body);
    Ok(Value::Unit)
}

/// `time::sleep(duration)` — block the thread for the given
/// duration. v0: the interpreter is single-threaded so this is
/// just a real OS sleep. Replace with the cooperative scheduler
/// in Phase 2.
pub fn time_sleep(args: &[Value]) -> Result<Value, String> {
    let ns = match args.first() {
        Some(Value::Duration(ns)) => *ns,
        Some(other) => {
            return Err(format!(
                "time::sleep expects a Duration, got {}",
                other.type_name()
            ));
        }
        None => return Err("time::sleep called with no arguments".to_string()),
    };
    if ns > 0 {
        thread::sleep(Duration::from_nanos(ns as u64));
    }
    Ok(Value::Unit)
}

pub fn resolve_path(segments: &[&str]) -> Option<Value> {
    match segments {
        ["time", "sleep"] => Some(Value::Builtin(BuiltinRef {
            name: "time::sleep",
            func: Rc::new(time_sleep),
        })),
        _ => None,
    }
}
