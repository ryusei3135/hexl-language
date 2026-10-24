//! プリプロセッサ
//!
//! 命令ごとにファイルを分けている(`preprocessor/`)。
//! - `define.rs`: `#define`(値・関数形式・複数行)の読み取りと展開
//! - `include.rs`: `#include`
//! - `cond.rs`: `#if` / `#elif` / `#else` / `#endif`
//! - `line.rs`: `#line`
//!
//! このファイルには、登録済みの定義の表(`Preprocessor`)と、
//! ソースを先頭から読んで各命令へ振り分ける処理が入っている。

mod cond;
mod define;
mod include;
mod line;

use self::cond::{cond_else, cond_endif, is_active, CondFrame};
use self::define::read_multiline_define;
use self::include::is_kept_directive;
use super::comment::strip_comments;
use std::collections::HashMap;

/// `#define NAME VALUE` の右辺として実際に登録される値。
/// 今後 `define` 以外のプリプロセッサ命令が増える前提のため、
/// 素の `String` ではなく列挙体として持たせている。
/// (※ `crate::compiler::models` に既に同名の型がある場合はそちらに合わせて削除してください)
#[derive(Clone, Debug, PartialEq)]
pub(in crate::compiler::hexl_frontend::lex) enum ReplaceVal {
    Str(String),
}

#[cfg(target_os = "windows")]
const DEFAULT: &str = "windows";
/// windows 以外は linux 扱い
#[cfg(not(target_os = "windows"))]
const DEFAULT: &str = "linux";

/// 組み込みの OS シンボル。`--windows` / `--linux` で選ばれた1つだけが定義される。
/// (`#if #windows` / `#if #linux` / `#elif #linux` で使う)
const OS_NAMES: [&str; 2] = ["linux", "windows"];

#[derive(Debug)]
pub(in crate::compiler::hexl_frontend::lex) enum PreprocKind {
    /// `#define NAME VALUE`
    Define(ReplaceVal),
    /// `#define NAME(arg, arg2, ..) BODY`
    ///
    /// `#NAME(a, b)`で呼び出すと、`BODY`の中の仮引数(`arg`など)を
    /// 実引数のテキストに置き換えたものに展開される。
    ///
    /// `#define NAME{arg, arg2, ..} BODY`(`brace`が`true`)は、`()`の代わりに
    /// `{}`で引数を囲む形式で、`#NAME { a, b }`と呼び出す
    DefineFn {
        params: Vec<String>,
        body: String,
        brace: bool,
    },
}

#[derive(Debug)]

pub(in crate::compiler::hexl_frontend::lex) struct Preprocessor {
    tables: HashMap<String, PreprocKind>,
    /// `#include "file.c"` / `#include "file.h"` で指定された、C言語のファイル名
    /// (書かれたとおりの名前。読み込みや展開はせず、名前を保存するだけ。
    /// `Lexer`が`take_c_files`で受け取り、`include_c_file_list`に入れる)
    c_files: Vec<String>,
}

impl Preprocessor {
    pub(in crate::compiler::hexl_frontend::lex) fn new() -> Self {
        Self {
            tables: HashMap::new(),
            c_files: Vec::new(),
        }
    }

    /// これまでの`#include`で集まったC言語のファイル名を取り出す(取り出した分は空になる)
    pub(in crate::compiler::hexl_frontend::lex) fn take_c_files(&mut self) -> Vec<String> {
        std::mem::take(&mut self.c_files)
    }

    /// 実行環境(`DEFAULT`)のOSシンボルを定義する。
    pub fn setup_defines(self) -> Self {
        self.setup_defines_for(DEFAULT)
    }

    /// `os`(`"linux"` / `"windows"`)のシンボルだけを定義する。
    /// もう一方のOSは未定義のままなので、`#if #windows` などの条件で区別できる。
    /// 知らないOS名の場合は何も定義しない(両方未定義)。
    pub fn setup_defines_for(mut self, os: &str) -> Self {
        if let Some(name) = OS_NAMES.iter().find(|n| **n == os) {
            self.tables.insert(
                name.to_string(),
                PreprocKind::Define(ReplaceVal::Str(name.to_string())),
            );
        }
        self
    }

    /// 名前がまだ登録されていなければ追加する。
    /// 仕様には書かれていないが、同名の再定義は無視する
    /// (元の `add` の `is_none()` チェックを踏襲)。
    pub(in crate::compiler::hexl_frontend::lex) fn add(
        &mut self,
        name: &str,
        preproc: PreprocKind,
    ) {
        if self.tables.get(name).is_none() {
            self.tables.insert(name.to_string(), preproc);
        }
    }

    /// `#NAME` に登録されている内容を返す。
    /// 仕様どおり、登録されていない名前が来たら panic する。
    pub(in crate::compiler::hexl_frontend::lex) fn resolve(&self, name: &str) -> &PreprocKind {
        match self.tables.get(name) {
            Some(kind) => kind,
            None => panic!("undefined preprocessor symbol: `{}`", name),
        }
    }

    /// `#if #NAME` 用: panicせずに「定義済みかどうか」だけを調べる。
    pub(in crate::compiler::hexl_frontend::lex) fn is_defined(&self, name: &str) -> bool {
        self.tables.get(name).is_some()
    }

    /// プリプロセッサを処理し、展開済みのソースを作る。
    ///
    /// 対応する行頭ディレクティブ(命令ごとの処理は`preprocessor/`の各ファイル):
    /// - `#define NAME VALUE`: `Preprocessor` に登録する
    /// - `#define NAME {{ ... }}` / `#define NAME(args..) {{ ... }}`:
    ///   `{{`から最初の`}}`までを本体として、複数行にまたがって書ける
    /// - `#define NAME{arg, arg2, ..} BODY`: `()`の代わりに`{}`を使う関数形式マクロ。
    ///   `#NAME { a, b }`と呼び出す(`{{ ... }}`の複数行の本体も使える)
    /// - `#define NAME(arg, arg2, ..) BODY`: 関数形式マクロとして登録する。
    ///   `#NAME(a, b)`と書くと、`BODY`の仮引数を実引数に置き換えて展開する
    ///   (名前と`(`の間に空白を入れると、`NAME VALUE`の形式になる)
    /// - `#if #NAME` / `#elif #NAME` / `#else` / `#endif`: 上から順に条件を調べ、
    ///   最初に `NAME` が定義済みだった枝だけを有効にする(どれも当てはまらなければ
    ///   `#else` の枝)。`#elif`は何個でも、`#else`は省略可、入れ子可。
    ///   `#windows` / `#linux` は `--windows` / `--linux` で選んだOSだけが定義済みになる
    /// - `#include "file.c"`: C言語のファイル名を保存する(`.hexl`は後段に任せる)
    /// - `#line`: 元のソースの行番号に置き換わる
    pub(in crate::compiler::hexl_frontend::lex) fn preprocess(
        &mut self,
        content: &str,
    ) -> Option<String> {
        Some(self.preprocess_text(content))
    }

    /// `preprocess` の本体
    fn preprocess_text(&mut self, content: &str) -> String {
        // `//` と `/* */` のコメントを先に取り除く。
        // (`#include`で取り込んだファイルもここを通るので、そちらのコメントも消える。
        //  改行は残すので、行番号はずれない)
        let content = strip_comments(content);
        let chars: Vec<char> = content.chars().collect();
        let mut output = String::new();
        let mut i = 0;
        // 現在位置までの行内が空白だけ(＝まだ行頭)かどうか
        let mut at_line_start = true;
        // 元ソース上で、今どの行を処理しているか(`#line`用)
        let mut current_line: usize = 1;
        let mut cond_stack: Vec<CondFrame> = Vec::new();

        while i < chars.len() {
            let c = chars[i];

            if c == '#' {
                let name_start = i + 1;
                let mut j = name_start;
                while j < chars.len() && is_name_char(chars[j]) {
                    j += 1;
                }

                if j > name_start {
                    let name: String = chars[name_start..j].iter().collect();

                    if at_line_start {
                        let mut line_end = j;
                        while line_end < chars.len() && chars[line_end] != '\n' {
                            line_end += 1;
                        }
                        let rest_of_line: String = chars[j..line_end].iter().collect();
                        let active_here = is_active(&cond_stack);

                        // `#define NAME {{ ... }}`: `}}`までを1つの定義として読む(複数行可)。
                        // `line_end`は、`}}`がある行の末尾になる
                        let multiline = if name == "define" {
                            read_multiline_define(&chars, j, line_end)
                        } else {
                            None
                        };
                        let (rest_of_line, line_end) = match multiline {
                            Some((text, end)) => (text, end),
                            None => (rest_of_line, line_end),
                        };
                        // 定義の中で読み飛ばす改行の数(単一行の定義は0)
                        let skipped_lines =
                            chars[j..line_end].iter().filter(|c| **c == '\n').count();

                        match name.as_str() {
                            "if" => {
                                self.cond_if(&mut cond_stack, &rest_of_line, active_here);
                                i = line_end;
                                continue;
                            }
                            "elif" => {
                                self.cond_elif(&mut cond_stack, &rest_of_line);
                                i = line_end;
                                continue;
                            }
                            "else" => {
                                cond_else(&mut cond_stack);
                                i = line_end;
                                continue;
                            }
                            "endif" => {
                                cond_endif(&mut cond_stack);
                                i = line_end;
                                continue;
                            }
                            "include" => {
                                if self.include_directive(&rest_of_line, active_here) {
                                    i = line_end;
                                    continue;
                                }
                            }
                            "define" => {
                                if self.define_directive(
                                    &rest_of_line,
                                    active_here,
                                    skipped_lines,
                                    &mut output,
                                    &mut current_line,
                                ) {
                                    i = line_end;
                                    continue;
                                }
                            }
                            _ => {}
                        }
                    }

                    // 行頭ディレクティブでなければ、インライン展開として扱う
                    // (`next`は、展開で読み飛ばした分を進めた後の位置)
                    let mut next = j;
                    if is_active(&cond_stack) {
                        if is_kept_directive(&name) {
                            // `#asm(...)` は構文として残し、後段のパーサーが
                            // 直接解釈する。ここで未定義マクロ扱いにしてはならない。
                            output.push('#');
                            output.push_str(&name);
                        } else if name == line::NAME {
                            output.push_str(&line::line_text(current_line));
                        } else {
                            next =
                                self.expand_call(&name, &chars, j, &mut output, &mut current_line);
                        }
                    }
                    i = next;
                    at_line_start = false;
                    continue;
                }
            }

            if is_active(&cond_stack) {
                output.push(c);
            }
            if c == '\n' {
                current_line += 1;
            }
            at_line_start = c == '\n' || (at_line_start && c.is_whitespace());
            i += 1;
        }

        output
    }
}

/// 識別子に使える文字か
pub(super) fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}
