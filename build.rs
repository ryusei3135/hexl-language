use std::process::Command;
use std::{env, fs, path::Path};

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
    let mut body_info = String::new();
    let mut body_api = String::new();

    type_info_head.push_str("use super::*;\n\n");
    type_info_head.push_str("create_var_type_data!(\n");
    type_api_head.push_str("use super::*;\n\n");
    type_api_head.push_str("create_type_api!(\n");

    for line in src.lines() {
        let parts: Vec<_> = line.split_whitespace().collect();
        let name = &parts[0];
        let fields = &parts[1];
        let txt = &parts[2];

        if fields.is_empty() {
            body_info.push_str(&format!("    {},\n", name));
        } else {
            body_info.push_str(&format!(
                "    {}: {}",
                name,
                fields
            ));
            body_api.push_str(&format!(
                "    {}: {}",
                name,
                fields
            ));
            body_api.push_str(&format!(
                " : {},\n",
                txt
            ));
            body_info.push_str(",\n");
        }
    }

    type_info_head.push_str(&body_info);
    type_info_head.push_str(");\n");
    type_api_head.push_str(&body_api);
    type_api_head.push_str(");\n");
    println!("build.rs cwd = {:?}", std::env::current_dir());
    println!("trying to access file here");

    let out = Path::new("src");
    fs::write(out.join("manager/type_info.rs"), type_info_head).expect("manager/type_info.rs");
    fs::write(out.join("api/type_api.rs"), type_api_head).expect("api/type_api.rs");
}


fn main() {
    let status = Command::new("python3")
        .args(&["make_c_value.py"])
        .status()
        .expect("failed to run python");

    if !status.success() {
        eprintln!("python script failed");
    }

    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_path = Path::new(&crate_dir).join("std_lib/vm.h");

    cbindgen::generate(&crate_dir)
        .expect("Unable to generate C bindings")
        .write_to_file(out_path);

    // 再生成条件
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=cbindgen.toml");
    println!("cargo:rerun-if-changed=src/type.dsl");

    let status = Command::new("python3")
        .args(&["lib_build.py"])
        .status()
        .expect("failed to run python");

    if !status.success() {
        eprintln!("python script failed");
    }
    create_type_data();
}
