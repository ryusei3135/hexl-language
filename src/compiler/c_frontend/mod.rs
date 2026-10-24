mod lex;
mod parse;

use crate::compiler::ast::Group1Node;
use parse::parse_c;
use std::{fmt, path::PathBuf};

/// Cフロントエンドの設定(コンパイラの`-I` `-isystem` `-D` `-U`相当)
#[derive(Clone, Debug, Default)]
pub struct COptions {
    /// `-I dir`(`#include "..."`と`<...>`の両方で探す)
    pub include_dirs: Vec<PathBuf>,
    /// システムヘッダのディレクトリ(`include_dirs`の後に探す)
    pub system_include_dirs: Vec<PathBuf>,
    /// `-DNAME=VALUE`(`("NAME", "VALUE")`)。関数形式は`("F(x)", "x+1")`
    pub defines: Vec<(String, String)>,
    /// `-UNAME`(`defines`の後に適用する)
    pub undefs: Vec<String>,
    /// メモリ上のヘッダ`(名前, 中身)`(ファイルより優先)
    pub virtual_files: Vec<(String, String)>,
}

/// 変換結果
#[derive(Clone, Debug)]
pub struct COutput {
    pub nodes: Vec<Group1Node>,
    /// 読み飛ばした・近似したもの(プリプロセッサの`#warning`も含む)
    pub warnings: Vec<String>,
    /// 解釈しなかった`#pragma`の本文
    pub pragmas: Vec<String>,
}

/// プリプロセス・構文解析・変換のどこかで起きたエラー(位置つきの文)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CFrontendError(pub String);

impl fmt::Display for CFrontendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for CFrontendError {}

fn build_preprocessor(opts: &COptions) -> Result<lex::Preprocessor, CFrontendError> {
    let mut pp = lex::Preprocessor::new();
    for d in &opts.include_dirs {
        pp.add_include_dir(d);
    }
    for d in &opts.system_include_dirs {
        pp.add_system_include_dir(d);
    }
    for (name, contents) in &opts.virtual_files {
        pp.add_virtual_file(name, contents);
    }
    for (name, value) in &opts.defines {
        pp.define(name, value)
            .map_err(|e| CFrontendError(e.to_string()))?;
    }
    for name in &opts.undefs {
        pp.undef(name);
    }
    Ok(pp)
}

fn finish(
    pp: &lex::Preprocessor,
    tokens: Vec<lex::TokenInfo>,
) -> Result<COutput, CFrontendError> {
    let out = parse_c(tokens).map_err(|e| CFrontendError(e.to_string()))?;
    let mut warnings = pp.warnings().to_vec();
    warnings.extend(out.warnings);
    Ok(COutput {
        nodes: out.nodes,
        warnings,
        pragmas: pp.pragmas().to_vec(),
    })
}

/// Cのソース文字列を変換する。`file_name`はエラー表示と`__FILE__`に使う
pub fn try_emit_c_node(
    file_name: &str,
    src: &str,
    opts: &COptions,
) -> Result<COutput, CFrontendError> {
    let mut pp = build_preprocessor(opts)?;
    let tokens = pp
        .preprocess(file_name, src)
        .map_err(|e| CFrontendError(e.to_string()))?;
    finish(&pp, tokens)
}

/// Cのファイルを変換する(`#include "x.h"`は、そのファイルのあるディレクトリからも探す)
pub fn try_emit_c_node_from_file(
    path: &str,
    opts: &COptions,
) -> Result<COutput, CFrontendError> {
    let mut pp = build_preprocessor(opts)?;
    let tokens = pp
        .preprocess_file(path)
        .map_err(|e| CFrontendError(e.to_string()))?;
    finish(&pp, tokens)
}

/// 警告を標準エラーへ出し、ノードだけ返す。エラーならメッセージつきでpanicする
fn report(result: Result<COutput, CFrontendError>) -> Vec<Group1Node> {
    match result {
        Ok(out) => {
            for w in &out.warnings {
                eprintln!("warning: {w}");
            }
            for p in &out.pragmas {
                eprintln!("note: 解釈しなかった#pragma: {p}");
            }
            out.nodes
        }
        Err(e) => panic!("{e}"),
    }
}

/// c言語のコードを変換
pub fn emit_c_node(src: &str) -> Vec<Group1Node> {
    report(try_emit_c_node("<input>", src, &COptions::default()))
}

/// c言語のファイルを変換(`#include "x.h"`を、ファイルのあるディレクトリから解決できる)
pub fn emit_c_node_from_file(path: &str) -> Vec<Group1Node> {
    report(try_emit_c_node_from_file(path, &COptions::default()))
}
