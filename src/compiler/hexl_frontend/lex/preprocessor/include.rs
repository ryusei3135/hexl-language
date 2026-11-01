//! `#include`の処理
//!
//! `.hexl`は展開せず、`#include`のまま後段(IR)に任せる。
//! `.c` / `.h`だけは、ここでファイル名を保存する。

use super::Preprocessor;
use crate::compiler::hexl_frontend::include_root;

impl Preprocessor {
    /// `#include`の行(`#include`の直後からの文字列`rest_of_line`)を処理する。
    ///
    /// `#include "file.c"` / `#include "file.h"` だけは、ここで名前を保存する(展開しない)。
    /// それ以外(`.hexl`)は展開せず、`#include`のまま後段に任せる。
    /// 後段は`ROOT/dir/file`として読み込み、`pub`の付いた関数をIRへ取り込み、
    /// 公開型ノードは呼び出し側のASTへ取り込む。
    ///
    /// 名前を保存して、この行の処理を終えたら`true`
    /// (無効な`#if`の枝の中の`#include`は処理しないので`false`)
    pub(super) fn include_directive(&mut self, rest_of_line: &str, active: bool) -> bool {
        // 無効な`#if`の枝の中なら、名前は保存しない
        if !active {
            return false;
        }
        if let Some(rel) = plain_include_path(rest_of_line) {
            if rel.ends_with(".c") || rel.ends_with(".h") {
                // `#define`を集めている間は保存しない
                // (`.hexl`は別のファイルとして処理するときに、そちらが保存する)
                if !self.collect_only && !self.c_files.iter().any(|f| *f == rel) {
                    self.c_files.push(rel);
                }
                return true;
            }
        }

        // `.hexl`は展開せず後段に任せるが、そのファイルの`#define`は集めて登録する
        if let Some((rel, is_std)) = include_target(rest_of_line) {
            if rel.ends_with(".hexl") {
                self.collect_defines(&rel, is_std);
            }
        }

        false
    }

    /// `#include`先のファイル`rel`にある`#define`を全て集めて、登録する。
    ///
    /// - ファイルは、ソースの出力に含めない(`.hexl`の展開は後段(IR)が別ファイルとして行う)。
    ///   ここでは、`#define`の登録と、それに必要な`#if` / `#elif` / `#else` / `#endif`の判定だけを行う。
    /// - 入れ子の`#include`(取り込んだファイルが、さらに`#include`するファイル)も、
    ///   同じように辿る。同じファイルは1回しか読まない(循環しても止まる)。
    /// - 同名の`#define`は、先に登録されたものが残る(`add`と同じ)。
    ///   `#include`より前の定義が優先される。
    /// - ファイルが読めない場合は、何もしない(「ファイルが無い」エラーは後段(IR)が出す)。
    /// - パスは`include_root::resolve`(`std::"..."`は`resolve_std`)で解決する。
    fn collect_defines(&mut self, rel: &str, is_std: bool) {
        let path = if is_std {
            include_root::resolve_std(rel)
        } else {
            include_root::resolve(rel)
        };
        // すでに集めた(または集めている途中の)ファイルは読まない
        if !self.collected_files.insert(path.clone()) {
            return;
        }
        let Ok(content) = std::fs::read_to_string(&path) else {
            return;
        };

        let was_collecting = self.collect_only;
        self.collect_only = true;
        // 出力は使わない。`#define`の登録と、入れ子の`#include`の処理だけが目的
        let _ = self.preprocess_text(&content);
        self.collect_only = was_collecting;
    }
}

/// `#include`の後ろ(`rest_of_line`)から、ファイルのパスと、標準ライブラリ(`std::"..."`)かどうかを取り出す。
///
/// 次の書き方に対応する(`parse/preproc.rs`の`build_include_path`と同じ)。
/// - `"file"` / `"file"::func` / `"file"::*`
/// - `Name="file"`
/// - `std::"file"` / `Name=std::"file"`(標準ライブラリ)
///
/// 文字列を含まない`mod::file`の書き方は、ファイルのパスが分からないので`None`。
fn include_target(rest_of_line: &str) -> Option<(String, bool)> {
    let mut rest = rest_of_line.trim();

    // `Name=`
    let name_len = rest
        .find(|c: char| !(c.is_alphanumeric() || c == '_'))
        .unwrap_or(rest.len());
    if name_len > 0 {
        if let Some(after) = rest[name_len..].trim_start().strip_prefix('=') {
            rest = after.trim_start();
        }
    }

    // `std::`
    let mut is_std = false;
    if let Some(after) = rest.strip_prefix("std") {
        if let Some(after) = after.trim_start().strip_prefix("::") {
            let after = after.trim_start();
            if after.starts_with('"') {
                is_std = true;
                rest = after;
            }
        }
    }

    let after_quote = rest.strip_prefix('"')?;
    let close = after_quote.find('"')?;
    Some((after_quote[..close].to_string(), is_std))
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

#[cfg(test)]
mod collect_defines_tests {
    use super::*;
    use std::path::PathBuf;

    /// テスト用の一時ディレクトリに、`files`(パス, 中身)を作り、`main.hexl`を`ROOT`にする
    fn setup(name: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("hexl_collect_{}_{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for (path, content) in files {
            let file = dir.join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, content).unwrap();
        }
        include_root::set_root_from_file(&dir.join("main.hexl"));
        dir
    }

    fn pp() -> Preprocessor {
        Preprocessor::new().setup_defines_for("linux")
    }

    #[test]
    fn include_target_forms() {
        let t = |s: &str| include_target(s);
        assert_eq!(t(" \"a/b.hexl\""), Some(("a/b.hexl".to_string(), false)));
        assert_eq!(t(" \"a.hexl\"::func"), Some(("a.hexl".to_string(), false)));
        assert_eq!(t(" \"a.hexl\"::*"), Some(("a.hexl".to_string(), false)));
        assert_eq!(t(" io=\"a.hexl\""), Some(("a.hexl".to_string(), false)));
        assert_eq!(
            t(" std::\"os/v.hexl\""),
            Some(("os/v.hexl".to_string(), true))
        );
        assert_eq!(
            t(" io=std::\"os/v.hexl\""),
            Some(("os/v.hexl".to_string(), true))
        );
        // 文字列が無い従来の書き方は、パスが分からない
        assert_eq!(t(" mod::file"), None);
    }

    #[test]
    fn collects_defines_of_included_file() {
        let dir = setup(
            "basic",
            &[(
                "a.hexl",
                "#define A 10\n#define F(x) x + 1\npub f(): int { ret 0 }\n",
            )],
        );
        let mut p = pp();
        let out = p.preprocess("#include \"a.hexl\"\nret #A + #F(2)").unwrap();
        // `#include`は展開せずに残し、`#define`だけを取り込む
        assert!(out.contains("#include \"a.hexl\""), "{}", out);
        assert!(out.contains("ret 10 + 2 + 1"), "{}", out);
        assert!(!out.contains("pub f()"), "{}", out);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn collects_nested_includes_and_stops_on_cycle() {
        let dir = setup(
            "nested",
            &[
                ("a.hexl", "#include \"b.hexl\"\n#define A 1\n"),
                ("b.hexl", "#define B 2\n#include \"a.hexl\"\n"),
            ],
        );
        let mut p = pp();
        let out = p.preprocess("#include \"a.hexl\"\n#A #B").unwrap();
        assert!(out.contains("1 2"), "{}", out);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn respects_conditions_and_first_definition_wins() {
        let dir = setup(
            "cond",
            &[(
                "a.hexl",
                "#if #windows\n#define OS 1\n#else\n#define OS 2\n#endif\n#define A 99\n",
            )],
        );
        let mut p = pp();
        let out = p
            .preprocess("#define A 5\n#include \"a.hexl\"\n#OS #A")
            .unwrap();
        // 無効な枝の`#define`は集めない。同名は先に定義した方が残る
        assert!(out.contains("2 5"), "{}", out);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn inactive_branch_and_missing_file_are_ignored() {
        let dir = setup("skip", &[("w.hexl", "#define W 1\n")]);
        let mut p = pp();
        // 無効な`#if`の中の`#include`は読まない。無いファイルは何もしない(後段が報告する)
        let out = p
            .preprocess("#if #windows\n#include \"w.hexl\"\n#endif\n#include \"none.hexl\"\n#if #W\nx\n#endif\n")
            .unwrap();
        assert!(!out.contains('x'), "{}", out);
        let _ = std::fs::remove_dir_all(dir);
    }
}
