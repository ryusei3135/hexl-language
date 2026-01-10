use std::env;
use std::path::Path;

fn main() {
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_path = Path::new(&crate_dir).join("std_lib/vm.h");

    cbindgen::generate(&crate_dir)
        .expect("Unable to generate C bindings")
        .write_to_file(out_path);

    // 再生成条件
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=cbindgen.toml");
}
