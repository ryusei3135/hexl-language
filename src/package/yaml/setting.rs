use serde::Deserialize;

/// ネイティブライブラリに関する情報
#[derive(Debug, Deserialize)]
pub struct NativeLibrary {
    pub metadata: LibraryFile,
    pub functions: Vec<Function>,
}

/// ライブラリファイルに関する情報
#[derive(Debug, Deserialize)]
pub struct LibraryFile {
    pub filename: String,
}

/// ライブラリ内の関数情報
#[derive(Debug, Deserialize)]
pub struct Function {
    pub name: String,
    pub args_types: Vec<String>,
    // pub return_type: String,
}
