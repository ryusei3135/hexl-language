use std::process::Command;
use std::{env, fs, path::Path};


fn compile_std_lib() {
    let cpp_file = "std_lib/io.cpp";
    // 出力先
    let target_dir = Path::new("extern_lib/std");
    fs::create_dir_all(target_dir).unwrap();
    let output_so = target_dir.join("io.so");

    Command::new("clang++")
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

//  変数の型に関する情報を作成
fn create_type_data() {
    let manifest = env::var("CARGO_MANIFEST_DIR").unwrap();
    let path = std::path::Path::new(&manifest).join("src/type.dsl");
    let src = fs::read_to_string(&path)
        .expect("failed to read src/type.dsl");
    //  manager/type_info.rs
    let mut type_info_head = String::new();
    //  api/type_api.rs
    let mut type_api_head = String::new();
    let mut body = String::new();

    type_info_head.push_str("use crate::create_var_type_data;\n\n");
    type_info_head.push_str("create_var_type_data!(\n");
    type_api_head.push_str("use super::*;\n\n");
    type_api_head.push_str("create_type_api!(\n");

    for line in src.lines() {
        let parts: Vec<_> = line.split_whitespace().collect();
        let name = parts[0];
        let fields = &parts[1..];

        if fields.is_empty() {
            body.push_str(&format!("    {},\n", name));
        } else {
            body.push_str(&format!(
                "    {}: {},\n",
                name,
                fields.join(", ")
            ));
        }
    }

    type_info_head.push_str(&body);
    type_info_head.push_str(");\n");
    type_api_head.push_str(&body);
    type_api_head.push_str(");\n");
    println!("build.rs cwd = {:?}", std::env::current_dir());
    println!("trying to access file here");

    let out = Path::new("src");
    fs::write(out.join("manager/type_info.rs"), type_info_head).expect("manager/type_info.rs");
    fs::write(out.join("api/type_api.rs"), type_api_head).expect("api/type_api.rs");
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
    println!("cargo:rerun-if-changed=src/type.dsl");

    compile_std_lib();
    create_type_data();
}
