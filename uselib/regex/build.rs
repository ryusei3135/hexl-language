use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let manifest_dir = PathBuf::from(
        env::var("CARGO_MANIFEST_DIR").unwrap()
    );

    let build_py = manifest_dir.join("build.py");

    Command::new("python3")
        .arg(&build_py)
        .arg("x64")
        .status()
        .expect("failed to execute build.py");

    println!(
        "cargo:rustc-link-search=native={}",
        manifest_dir.display()
    );

    println!("cargo:rustc-link-lib=static=regex");
    println!("cargo:rustc-link-search=native=uselib/regex");

    println!("cargo:rerun-if-changed=src/c");
    println!("cargo:rerun-if-changed=src/asm");
}