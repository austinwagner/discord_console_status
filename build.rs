extern crate serde_codegen;

use std::env;
use std::path::Path;

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();

    let src = Path::new("src/xbl/responses.in.rs");
    let dst = Path::new(&out_dir).join("xbl.responses.rs");
    serde_codegen::expand(&src, &dst).unwrap();

    let src = Path::new("src/psn/responses.in.rs");
    let dst = Path::new(&out_dir).join("psn.responses.rs");
    serde_codegen::expand(&src, &dst).unwrap();

    let src = Path::new("src/config/file.in.rs");
    let dst = Path::new(&out_dir).join("config.file.rs");
    serde_codegen::expand(&src, &dst).unwrap();
}
