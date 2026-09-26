mod convert;
pub mod elf;
mod emitter;
mod parse;

use emitter::Emitter;
use parse::{Lexer, Parser};

/// アセンブリソースをアセンブルしてファイルに書き出す。
///
/// - `mode` に `"-o"` を渡すと再配置可能オブジェクトファイル (`a.o`) を生成する。
/// - `mode` に `"-c"` を渡すと単独で実行可能な ELF 実行ファイル (`a.out`) を生成する。
/// - `source` には AT&T 構文の x64 アセンブリコードをそのまま渡す。
///
/// この関数はレキサ/パーサ/コード生成/ELF出力までをすべて内部で行うため、
/// 呼び出し側はモードとソースコードを渡すだけでよい。
pub fn emitter_x64(mode: &str, source: &str) -> Result<(), String> {
    let tokens = Lexer::new(source).tokenize();

    // 2. 構文解析
    let mut parser = Parser::new(tokens);
    let ast = parser.parse()?;

    // 3. パース結果 (文字列ベースのAST) を
    //    エミッタ用の型付きASTに変換する
    let program = convert::convert_program(&ast)?;

    // 4. 機械語の生成 + ラベル/シンボルの解決
    let mut emit = Emitter::new();
    emit.emit_program(&program).unwrap();
    emit.finish()?;

    // 5. モードに応じてファイルへ書き出す
    match mode {
        "-o" => {
            let bytes = elf::write_object(&emit)?;

            std::fs::write("a.o", &bytes)
                .map_err(|e| format!("a.o の書き込みに失敗しました: {}", e))?;

            println!("wrote a.o ({} bytes)", bytes.len());
        }

        "-c" => {
            let bytes = elf::write_executable(&emit)?;

            std::fs::write("a.out", &bytes)
                .map_err(|e| format!("a.out の書き込みに失敗しました: {}", e))?;

            make_executable("a.out")?;

            println!("wrote a.out ({} bytes)", bytes.len());
        }

        other => {
            return Err(format!(
                "不明なモードです: {} (\"-o\" または \"-c\" を指定してください)",
                other
            ));
        }
    }

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
