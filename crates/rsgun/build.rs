//! Brings in the port's generated code (tools/port/sh2rust.py writes build/port/generated.rs: it is
//! ROM-derived, so never committed) when it is there; without it the game runs on the interpreter.

use std::path::Path;

fn main() {
    let out = std::env::var("OUT_DIR").expect("OUT_DIR");
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/port/generated.rs");
    println!("cargo:rerun-if-changed={}", src.display());
    let dst = Path::new(&out).join("port.rs");
    let code = std::fs::read_to_string(&src).unwrap_or_else(|_| {
        "pub mod generated { pub fn register(_: &mut stv::native::Natives) {} }\n".to_string()
    });
    std::fs::write(dst, code).expect("port.rs");
}
