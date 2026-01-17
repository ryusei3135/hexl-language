use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;


fn compile_std_lib() {
    let cpp_file = "std_lib/io.cpp";
    // 出力先
    let target_dir = Path::new("extern_lib/std");
    fs::create_dir_all(target_dir).unwrap();
    let output_so = target_dir.join("io.so");

    let status = Command::new("clang++")
            .args(&[
                "-shared",         // 動的ライブラリ
                "-fPIC",           // 位置独立コード
                "-std=c++17",      // C++17
                cpp_file,
                "-o",
                output_so.to_str().unwrap(),
            ])
            .status()
            .expect("failed to execute g++");
}


fn main() {
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_path = Path::new(&crate_dir).join("std_lib/vm.h");

    cbindgen::generate(&crate_dir)
        .expect("Unable to generate C bindings")
        .write_to_file(out_path);

    // 再生成条件
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=cbindgen.toml");

    compile_std_lib();
}
