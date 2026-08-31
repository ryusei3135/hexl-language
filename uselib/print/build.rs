use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let manifest_dir = PathBuf::from(
        env::var("CARGO_MANIFEST_DIR").unwrap()
    );

    // uselib/print/Makefile を実行
    let status = Command::new("make")
        .current_dir(&manifest_dir)
        .status()
        .expect("failed to execute make");

    if !status.success() {
        panic!("failed to build native runtime");
    }

    // libruntime.a がある場所
    println!(
        "cargo:rustc-link-search=native={}",
        manifest_dir.display()
    );

    println!("cargo:rustc-link-lib=static=runtime");

    println!("cargo:rerun-if-changed=Makefile");
    println!("cargo:rerun-if-changed=c/print.c");
    println!("cargo:rerun-if-changed=asm/linux_write.s");
}