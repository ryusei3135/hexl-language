//! `#include`の処理
//!
//! `.hexl`は展開せず、`#include`のまま後段(IR)に任せる。
//! `.c` / `.h`だけは、ここでファイル名を保存する。

use super::Preprocessor;

impl Preprocessor {
    /// `#include`の行(`#include`の直後からの文字列`rest_of_line`)を処理する。
    ///
    /// `#include "file.c"` / `#include "file.h"` だけは、ここで名前を保存する(展開しない)。
    /// それ以外(`.hexl`)は展開せず、`#include`のまま後段に任せる。
    /// 後段は`ROOT/dir/file`として読み込み、`pub`の付いた
    /// 関数だけを取り込む(`include_root`、`ir/builder/preproc.rs`)。
    ///
    /// 名前を保存して、この行の処理を終えたら`true`
    /// (無効な`#if`の枝の中の`#include`は処理しないので`false`)
    pub(super) fn include_directive(&mut self, rest_of_line: &str, active: bool) -> bool {
        if active {
            return false;
        }
        if let Some(rel) = plain_include_path(rest_of_line) {
            if rel.ends_with(".c") || rel.ends_with(".h") {
                if !self.c_files.iter().any(|f| *f == rel) {
                    self.c_files.push(rel);
                }
                return true;
            }
        }
        
        false
    }
}

/// 展開せず、構文として残す`#NAME`か
/// (`#asm(...)`は後段のパーサーが直接解釈する。`#include`は上のとおり。
/// ここで未定義マクロ扱いにしてはならない)
pub(super) fn is_kept_directive(name: &str) -> bool {
    name == "asm" || name == "include"
}

/// `#include` の後ろ(`rest_of_line`)が `"file"` だけ(後ろは空白か `//` のコメントのみ)なら
/// `file` を返す。`"file"::io` や `io="file"` の形なら `None`。
fn plain_include_path(rest_of_line: &str) -> Option<String> {
    let rest = rest_of_line.trim();
    let after_quote = rest.strip_prefix('"')?;
    let close = after_quote.find('"')?;
    let tail = after_quote[close + 1..].trim();
    if tail.is_empty() || tail.starts_with("//") {
        Some(after_quote[..close].to_string())
    } else {
        None
    }
}
