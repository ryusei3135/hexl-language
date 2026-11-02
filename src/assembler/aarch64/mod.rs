mod directives;
pub(in crate::assembler) mod elf;
pub(in crate::assembler) mod emitter;
mod format_table;
mod parse;
mod reg;

use emitter::Emitter;
use parse::{Lexer, Parser};

/// アセンブリソースをアセンブルしてファイルに書き出す。
///
/// - `mode` に `"-o"` を渡すと再配置可能オブジェクトファイル (`a.o`) を生成する。
///   (オブジェクトファイルは ELF 固定。`os` の値には依存しない)
/// - `mode` に `"-c"` を渡すと単独で実行可能なファイルを生成する。
///   出力フォーマットは `os` で選択する:
///     - `"linux"` -> ELF 実行ファイル (`a.elf`)
/// - `source` には gas 風の aarch64 アセンブリコードを渡す。
///   レジスタは `%x0` / `%w0` / `%sp`、即値は `$8` の形式。
///
/// この関数はレキサ/パーサ/コード生成/バイナリ出力までをすべて内部で行うため、
/// 呼び出し側はモード・ソースコード・対象OSを渡すだけでよい。
///
/// 出力ファイル名は形式ごとの既定名 (`a.o` など) になる。
/// 出力先を指定したい場合は [`emitter_aarch64_to`] を使う。
pub fn emitter_aarch64(mode: &str, source: &str, os: &str) -> Result<(), String> {
    emitter_aarch64_to(mode, source, os, None).map(|_| ())
}

/// [`emitter_aarch64`] と同じ処理を行い、出力先のパスを指定できる版。
///
/// - `out_path` が `Some(path)` ならそのパスへ、`None` なら形式ごとの既定名へ書き出す。
/// - 実際に書き出したファイルのパスを返す。
pub fn emitter_aarch64_to(
    mode: &str,
    source: &str,
    os: &str,
    out_path: Option<&str>,
) -> Result<String, String> {
    // 1. 字句解析 + 構文解析
    let tokens = Lexer::new(source).tokenize()?;
    let program = Parser::new(tokens).parse()?;

    // 2. 機械語の生成 + 同一セクション内のラベル解決
    let mut emit = Emitter::new();
    emit.emit_program(&program)?;
    emit.finish()?;

    // 3. モード/OSに応じてファイルへ書き出す
    //    (対応する出力形式は `format_table.rs` の表で管理している)
    let format = format_table::find(mode, os)?;

    let bytes = (format.write)(&emit)?;

    let file_name = out_path.unwrap_or(format.file_name);

    std::fs::write(file_name, &bytes)
        .map_err(|e| format!("{} の書き込みに失敗しました: {}", file_name, e))?;

    if format.executable {
        make_executable(file_name)?;
    }

    println!("wrote {} ({} bytes)", file_name, bytes.len());

    Ok(file_name.to_string())
}

/// アセンブルだけを行い、`.text` / `.data` のバイト列を返す (ファイルは書かない)。
/// テストや、他の処理に機械語を渡したいときに使う。
pub fn assemble_aarch64(source: &str) -> Result<(Vec<u8>, Vec<u8>), String> {
    let tokens = Lexer::new(source).tokenize()?;
    let program = Parser::new(tokens).parse()?;

    let mut emit = Emitter::new();
    emit.emit_program(&program)?;
    emit.finish()?;

    Ok((emit.text, emit.data))
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
