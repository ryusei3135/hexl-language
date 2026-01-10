fn main() {
    // プロジェクトルートを指定
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    cbindgen::generate(&crate_dir)
        .expect("Failed to generate C header with cbindgen")
        .write_to_file("std_lib/vm.h");
}
