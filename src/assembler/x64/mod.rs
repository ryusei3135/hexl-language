mod convert;
pub(in crate::assembler) mod emitter;
mod format_table;
mod parse;
mod reg;

use emitter::Emitter;
use parse::{Lexer, Parser};
pub use super::*;

/// アセンブリソースをアセンブルしてファイルに書き出す。
///
/// - `mode` に `"-o"` を渡すと再配置可能オブジェクトファイル (`a.o`) を生成する。
///   (オブジェクトファイルは ELF 固定。`os` の値には依存しない)
/// - `mode` に `"-c"` を渡すと単独で実行可能なファイルを生成する。
///   出力フォーマットは `os` で選択する:
///     - `"linux"` -> ELF 実行ファイル (`a.elf`)
///     - `"win"`   -> Windows PE32+ 実行ファイル (`a.exe`, Windows 11 でも動作)
/// - `source` には AT&T 構文の x64 アセンブリコードをそのまま渡す。
///
/// この関数はレキサ/パーサ/コード生成/バイナリ出力までをすべて内部で行うため、
/// 呼び出し側はモード・ソースコード・対象OSを渡すだけでよい。
pub fn emitter_x64(mode: &str, source: &str, os: &str) -> Result<(), String> {
    let tokens = Lexer::new(source).tokenize();

    // 2. 構文解析
    let mut parser = Parser::new(tokens);
    let ast = parser.parse()?;

    // 3. パース結果 (文字列ベースのAST) を
    //    エミッタ用の型付きASTに変換する
    let program = convert::convert_program(&ast)?;

    // 4. 機械語の生成 + ラベル/シンボルの解決
    let mut emit = Emitter::new();
    emit.emit_program(&program)?;
    emit.finish()?;

    // 5. モード/OSに応じてファイルへ書き出す
    //    (対応する出力形式は `format_table.rs` の表で管理している)
    let format = format_table::find(mode, os)?;

    let bytes = (format.write)(&emit)?;

    std::fs::write(format.file_name, &bytes)
        .map_err(|e| format!("{} の書き込みに失敗しました: {}", format.file_name, e))?;

    if format.executable {
        make_executable(format.file_name)?;
    }

    println!("wrote {} ({} bytes)", format.file_name, bytes.len());

    Ok(())
}

#[cfg(unix)]
fn make_executable(path: &str) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    let mut perms = std::fs::metadata(path)
        .map_err(|e| e.to_string())?
        .permissions();

    perms.set_mode(0o755);

    std::fs::set_permissions(path, perms).map_err(|e| e.to_string())
}

#[cfg(not(unix))]
fn make_executable(_path: &str) -> Result<(), String> {
    Ok(())
}
