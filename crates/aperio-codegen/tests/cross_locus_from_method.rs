//! Regression: cross-locus heap store from inside a locus method
//! body. Mirrors the cross_seed_locus_arg pattern but the
//! pushing call site is now inside `birth()` rather than `main()`.
//!
//! With Phase-4 per-method scratch, a free fn called from a method
//! has `__caller_arena = method's scratch`. The Entry literal built
//! inside that free fn lives in scratch. Pushing the pointer into
//! `reg.entries` (a `@form(vec)` on a different locus's arena)
//! records the scratch pointer; scratch destroys at method exit;
//! the vec then holds a dangler. We expect this to repro the bug
//! before any cross-arena-store hardening.
//!
//! If this test currently passes, the bug is latent (scratch happens
//! to not be reused before the .get(0)) but is real. If it fails
//! (segv / corruption / wrong values), we have a deterministic
//! repro and the fix is required.

use std::process::Command;
use aperio_codegen::build_executable;

fn build_and_run(name: &str, src: &str) -> (String, std::process::ExitStatus) {
    let program = aperio_syntax::parse_source(src).expect("parse");
    let mut bin = std::env::temp_dir();
    bin.push(format!(
        "aperio_cross_locus_method_{}_{}",
        name,
        std::process::id()
    ));
    build_executable(&program, &bin).expect("build");
    let out = Command::new(&bin).output().expect("run");
    let _ = std::fs::remove_file(&bin);
    (
        String::from_utf8_lossy(&out.stdout).to_string(),
        out.status,
    )
}

#[test]
fn push_via_freefn_from_method_body_survives_scratch_destroy() {
    let src = r#"
        type Entry { name: String; value: Int; }

        @form(vec)
        locus EntryList {
            capacity { heap items of Entry; }
        }

        locus Registry {
            params { entries: EntryList = EntryList { }; }
        }

        fn push_via_freefn(reg: Registry, n: String, v: Int) {
            reg.entries.push(Entry { name: n, value: v });
        }

        locus Caller {
            params { reg: Registry = Registry { }; }
            birth() {
                push_via_freefn(self.reg, "via-fn", 7);
            }
            run() {
                println("len=", to_string(self.reg.entries.len()));
                let e = self.reg.entries.get(0) or Entry { name: "FB", value: -1 };
                println("name=", e.name, " value=", to_string(e.value));
            }
        }

        fn main() { Caller { }; }
    "#;
    let (stdout, status) = build_and_run("push_then_get", src);
    assert!(status.success(), "non-zero (segv?): {:?}", status);
    assert!(stdout.contains("len=1"), "stdout: {:?}", stdout);
    assert!(stdout.contains("name=via-fn"), "stdout: {:?}", stdout);
    assert!(stdout.contains("value=7"), "stdout: {:?}", stdout);
}

#[test]
fn push_directly_from_method_body_survives_arena_clobber() {
    // No free fn — push from the method body itself. The Entry
    // literal is built in the method's scratch via current_arena_ptr.
    // Same dangling-pointer risk.
    let src = r#"
        type Entry { name: String; value: Int; }

        @form(vec)
        locus EntryList {
            capacity { heap items of Entry; }
        }

        locus Registry {
            params { entries: EntryList = EntryList { }; }
        }

        locus Caller {
            params { reg: Registry = Registry { }; }
            birth() {
                self.reg.entries.push(Entry { name: "direct", value: 11 });
            }
            run() {
                println("len=", to_string(self.reg.entries.len()));
                let e = self.reg.entries.get(0) or Entry { name: "FB", value: -1 };
                println("name=", e.name, " value=", to_string(e.value));
            }
        }

        fn main() { Caller { }; }
    "#;
    let (stdout, status) = build_and_run("direct_push", src);
    assert!(status.success(), "non-zero (segv?): {:?}", status);
    assert!(stdout.contains("len=1"), "stdout: {:?}", stdout);
    assert!(stdout.contains("name=direct"), "stdout: {:?}", stdout);
    assert!(stdout.contains("value=11"), "stdout: {:?}", stdout);
}

#[test]
fn hashmap_set_with_dynamic_string_from_method_survives() {
    // Same shape as the vec.push dynamic-String repro, but
    // against @form(hashmap). hashmap_set memcpys the value
    // struct into the slot; if heap-pointer fields still alias
    // method scratch, they dangle on method exit. The fix is
    // the same: deep-copy into the receiver's __arena before
    // the set.
    let src = r#"
        type Entry { id: Int; name: String; }

        @form(hashmap)
        locus EntryMap {
            capacity { pool entries of Entry indexed_by id; }
        }

        locus Caller {
            params { reg: EntryMap = EntryMap { }; }
            birth() {
                let nm = "dyn-" + to_string(42);
                self.reg.set(Entry { id: 7, name: nm });
            }
            run() {
                let mut i = 0;
                while i < 200 {
                    let _trash = "zzzzzzzzzzzzzzzz" + to_string(i);
                    i = i + 1;
                }
                let e = self.reg.get(7) or Entry { id: -1, name: "FB" };
                println("name=", e.name, " id=", to_string(e.id));
            }
        }

        fn main() { Caller { }; }
    "#;
    let (stdout, status) = build_and_run("hashmap_clobber", src);
    assert!(status.success(), "non-zero (segv?): {:?}", status);
    assert!(
        stdout.contains("name=dyn-42"),
        "hashmap entry's dynamic String content was clobbered \
         — cross-arena deep-copy at hashmap.set is missing.\nstdout: {:?}",
        stdout,
    );
    assert!(stdout.contains("id=7"), "stdout: {:?}", stdout);
}

#[test]
fn ring_buffer_push_with_dynamic_string_from_method_survives() {
    // @form(ring_buffer).push has the same memcpy shape. Heap
    // fields in the pushed value must anchor in the receiver's
    // arena, not the caller's scratch.
    let src = r#"
        type Frame { seq: Int; label: String; }

        @form(ring_buffer, cap = 16)
        locus FrameBuffer {
            capacity { pool history of Frame; }
        }

        locus Caller {
            params { frames: FrameBuffer = FrameBuffer { }; }
            birth() {
                let lbl = "evt-" + to_string(99);
                let _ok = self.frames.push(Frame { seq: 3, label: lbl });
            }
            run() {
                let mut i = 0;
                while i < 200 {
                    let _trash = "yyyyyyyyyyyyyyyy" + to_string(i);
                    i = i + 1;
                }
                println("len=", to_string(self.frames.len()));
            }
        }

        fn main() { Caller { }; }
    "#;
    let (stdout, status) = build_and_run("ring_clobber", src);
    assert!(status.success(), "non-zero (segv?): {:?}", status);
    // We don't fetch the entry back (ring_buffer doesn't expose
    // a get() per the synth surface listed in
    // try_lower_form_ring_buffer_method), but the survival of
    // the buffer itself across the clobber + the absence of a
    // crash on read of len() is the proof. A real consumer (e.g.
    // fathom's per-symbol last-N-corrupt-timestamps ring) would
    // hit the dangling read on its own consume path.
    assert!(stdout.contains("len=1"), "stdout: {:?}", stdout);
}

#[test]
fn push_with_dynamic_string_content_survives_scratch_clobber() {
    // The discriminating shape: the Entry.name is a DYNAMICALLY-
    // ALLOCATED String (concat result), not a literal. The string
    // bytes live in the method's scratch chunk. lotus_vec_push
    // memcpys the struct bytes (incl. the String ptr field) into
    // the vec's buffer in the receiver's arena — but the ptr still
    // aims at the scratch chunk. After birth exits, run() re-opens
    // a fresh subregion that likely reuses the same 64 KiB chunk
    // address, overwriting the freed chunk's bytes. The vec's
    // String pointer then resolves to garbage.
    //
    // Without a cross-arena deep-copy at vec.push, this test
    // catches the dangling-content bug. Pre-fix it surfaces
    // either a corrupted name or a crash.
    let src = r#"
        type Entry { name: String; value: Int; }

        @form(vec)
        locus EntryList {
            capacity { heap items of Entry; }
        }

        locus Registry {
            params { entries: EntryList = EntryList { }; }
        }

        locus Caller {
            params { reg: Registry = Registry { }; }
            birth() {
                // Dynamic String — lives in birth's scratch chunk.
                let nm = "dyn-" + to_string(42);
                self.reg.entries.push(Entry { name: nm, value: 1 });
            }
            run() {
                // Fresh scratch — likely the same 64 KiB chunk
                // address birth's chunk lived at. Allocate
                // enough to overwrite where `nm` used to live.
                let mut i = 0;
                while i < 200 {
                    let _trash = "zzzzzzzzzzzzzzzz" + to_string(i);
                    i = i + 1;
                }
                println("len=", to_string(self.reg.entries.len()));
                let e = self.reg.entries.get(0) or Entry { name: "FB", value: -1 };
                println("name=", e.name, " value=", to_string(e.value));
            }
        }

        fn main() { Caller { }; }
    "#;
    let (stdout, status) = build_and_run("clobber_dynamic", src);
    assert!(status.success(), "non-zero (segv?): {:?}", status);
    assert!(stdout.contains("len=1"), "stdout: {:?}", stdout);
    assert!(
        stdout.contains("name=dyn-42"),
        "Entry's dynamic String content was clobbered when the \
         method scratch was destroyed and reused — \
         cross-arena deep-copy at vec.push is missing.\nstdout: {:?}",
        stdout,
    );
    assert!(stdout.contains("value=1"), "stdout: {:?}", stdout);
}
