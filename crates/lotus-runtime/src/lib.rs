//! Lotus runtime — Phase 2.
//!
//! v0 cut: a tree-walking interpreter that runs parsed +
//! typechecked Lotus programs. Region allocator, cooperative
//! scheduler, and bus router come later in Phase 2; the
//! interpreter is the "is the language semantically real"
//! check that doesn't wait on codegen.
//!
//! Public surface:
//! - [`run_program`] / [`run_bundle`] — execute a parsed Program
//!   (or set of programs) starting from `fn main()`.

pub mod builtins;
pub mod bus;
pub mod env;
pub mod eval;
pub mod value;

pub use bus::{BusRouter, RingBuffer, SyncDispatch, Transport, TransportKind};
pub use eval::{run_bundle, run_bundle_with_bus, run_program};
pub use value::Value;

#[cfg(test)]
mod tests {
    use super::*;
    use lotus_syntax::parse_source;

    #[test]
    fn long_lived_closure_passes_at_program_end() {
        // SubscriberL has a bus subscribe (long-lived) and a
        // closure that should pass at program-end dissolve.
        let src = r#"
            type Ping { n: Int; }

            locus SubscriberL {
                params { count: Int = 0; }
                bus { subscribe "p" as on_ping of type Ping; }
                fn on_ping(p: Ping) {
                    self.count = self.count + 1;
                }
                closure stays_zero_or_more {
                    self.count ~~ 0 within 100;
                }
            }

            fn main() {
                SubscriberL { };
            }
        "#;
        let program = parse_source(src).unwrap();
        assert_eq!(run_program(&program).unwrap(), 0);
    }

    #[test]
    fn long_lived_closure_fails_at_program_end() {
        let src = r#"
            locus L {
                params { x: Int = 5; y: Int = 99; }
                bus { subscribe "_" as on_msg of type Int; }
                fn on_msg(_v: Int) { }
                closure xy_match {
                    self.x ~~ self.y within 0;
                }
            }
            fn main() { L { }; }
        "#;
        let program = parse_source(src).unwrap();
        let err = run_program(&program).expect_err("should fail at program end");
        assert!(
            err.contains("ClosureViolation") && err.contains("xy_match"),
            "expected ClosureViolation; got: {}",
            err
        );
    }
}
