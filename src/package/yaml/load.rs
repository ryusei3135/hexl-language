use std::fs;
use std::path::Path;
use crate::package::yaml::setting::*;

/// ネイティブライブラリの設定ファイルを読み込み、構造体に変換する
pub fn load_native_library_config(
    path: &Path,
) -> Result<NativeLibrary, Box<dyn std::error::Error>> {
    // ファイルを読み込む
    let yaml = fs::read_to_string(path)?;
    // YAML → Rust構造体へデシリアライズ
    let lib = serde_yaml::from_str::<NativeLibrary>(&yaml)?;

    // // デバッグ出力
    // println!("library file: {}", lib.metadata.filename);
    // for f in &lib.functions {
    //     println!("function: {}", f.name);
    // }

    Ok(lib)
}
