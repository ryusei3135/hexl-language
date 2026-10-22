use super::comment::strip_comments;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

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

/// 関数形式マクロの展開の入れ子の上限(`#define F(a) #F(a)`のような自己再帰の防止)
const MAX_EXPAND_DEPTH: usize = 64;

pub(in crate::compiler::hexl_frontend::lex) enum PreprocKind {
    /// `#define NAME VALUE`
    Define(ReplaceVal),
    /// `#define NAME(arg, arg2, ..) BODY`
    ///
    /// `#NAME(a, b)`で呼び出すと、`BODY`の中の仮引数(`arg`など)を
    /// 実引数のテキストに置き換えたものに展開される
    DefineFn { params: Vec<String>, body: String },
}

pub(in crate::compiler::hexl_frontend::lex) struct Preprocessor {
    tables: HashMap<String, PreprocKind>,
    /// `#include "file"` の相対パスの基準になるディレクトリ
    /// (読み込み中のファイルがあるディレクトリ。未設定なら実行時のカレント)
    base_dir: PathBuf,
    /// 今インライン展開中のファイルの正規化したパス(循環した `#include` の検出用)
    include_stack: Vec<PathBuf>,
    /// `#include "file.c"` / `#include "file.h"` で指定された、C言語のファイル名
    /// (書かれたとおりの名前。読み込みや展開はせず、名前を保存するだけ。
    /// `Lexer`が`take_c_files`で受け取り、`include_c_file_list`に入れる)
    c_files: Vec<String>,
}

impl Preprocessor {
    pub(in crate::compiler::hexl_frontend::lex) fn new() -> Self {
        Self {
            tables: HashMap::new(),
            base_dir: PathBuf::new(),
            include_stack: Vec::new(),
            c_files: Vec::new(),
        }
    }

    /// これまでの`#include`で集まったC言語のファイル名を取り出す(取り出した分は空になる)
    pub(in crate::compiler::hexl_frontend::lex) fn take_c_files(&mut self) -> Vec<String> {
        std::mem::take(&mut self.c_files)
    }

    /// これから処理するソースファイルの場所を教える。
    /// `#include "file"` の相対パスは、このファイルのあるディレクトリが基準になる。
    pub(in crate::compiler::hexl_frontend::lex) fn set_source_file(&mut self, file: &Path) {
        self.base_dir = file.parent().map(Path::to_path_buf).unwrap_or_default();
        self.include_stack.clear();
        if let Ok(canon) = fs::canonicalize(file) {
            self.include_stack.push(canon);
        }
    }

    /// `#include "rel"` のファイルを読み込み、プリプロセッサを通した結果を返す。
    /// 取り込んだファイルの `#define` はそのまま以降のソースでも使える
    /// (`#if` の状態は、ファイルの中で閉じる)。
    ///
    /// ファイルが読めない場合と、`#include` が循環している場合は panic する。
    ///
    /// `.c` / `.h` は読み込まず、ファイル名だけを`c_files`に保存して、
    /// 空の文字列を返す(ソースには何も展開しない)。
    fn include_inline(&mut self, rel: &str) -> String {
        if rel.ends_with(".c") || rel.ends_with(".h") {
            if !self.c_files.iter().any(|f| f == rel) {
                self.c_files.push(rel.to_string());
            }
            String::new()
        } else {
            let path = self.base_dir.join(rel);
            let canon = fs::canonicalize(&path).unwrap_or_else(|e| {
                panic!(
                    "`#include \"{}\"`: cannot open `{}`: {}",
                    rel,
                    path.display(),
                    e
                )
            });
            if self.include_stack.contains(&canon) {
                panic!(
                    "`#include \"{}\"`: circular include of `{}`",
                    rel,
                    canon.display()
                );
            }
            let text = fs::read_to_string(&canon).unwrap_or_else(|e| {
                panic!(
                    "`#include \"{}\"`: cannot read `{}`: {}",
                    rel,
                    canon.display(),
                    e
                )
            });

            // 取り込んだファイルの中の `#include` は、そのファイルの場所が基準
            let saved_base = std::mem::replace(
                &mut self.base_dir,
                canon.parent().map(Path::to_path_buf).unwrap_or_default(),
            );
            self.include_stack.push(canon);
            let out = self.preprocess_text(&text);
            self.include_stack.pop();
            self.base_dir = saved_base;
            out
        }
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
    pub(in crate::compiler::hexl_frontend::lex) fn add(&mut self, name: &str, preproc: PreprocKind) {
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

    /// 関数形式マクロ`#name(args..)`を展開する。
    ///
    /// 1. 実引数の中の`#NAME`を先に展開する
    /// 2. `body`の中の仮引数を実引数のテキストに置き換える(文字列リテラルの中は置き換えない)
    /// 3. 置き換えた結果の中の`#NAME`(他のマクロの呼び出し)を展開する
    ///
    /// 実引数の数が仮引数の数と違う場合、入れ子が深すぎる場合は panic する。
    fn expand_fn_macro(
        &self,
        name: &str,
        params: &[String],
        body: &str,
        args: &[String],
        line: usize,
        depth: usize,
    ) -> String {
        if depth > MAX_EXPAND_DEPTH {
            panic!(
                "preprocessor macro `{}` is expanded too deeply (recursive?)",
                name
            );
        }
        // `#F()`は、実引数が空の1つではなく0個として扱う
        let args: &[String] = if params.is_empty() && args.len() == 1 && args[0].is_empty() {
            &[]
        } else {
            args
        };
        if params.len() != args.len() {
            panic!(
                "preprocessor macro `{}` takes {} argument(s) but {} were given",
                name,
                params.len(),
                args.len()
            );
        }
        let expanded_args: Vec<String> = args
            .iter()
            .map(|arg| self.expand_inline(arg, line, depth + 1))
            .collect();
        let replaced = substitute_params(body, params, &expanded_args);
        self.expand_inline(&replaced, line, depth + 1)
    }

    /// テキストの中の`#NAME`/`#NAME(args..)`を展開する
    /// (マクロの実引数と、置き換え後の本体に使う)
    fn expand_inline(&self, text: &str, line: usize, depth: usize) -> String {
        let chars: Vec<char> = text.chars().collect();
        let mut out = String::new();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '#' {
                let name_start = i + 1;
                let mut j = name_start;
                while j < chars.len() && is_name_char(chars[j]) {
                    j += 1;
                }
                if j > name_start {
                    let name: String = chars[name_start..j].iter().collect();
                    if name == "asm" || name == "include" {
                        out.push('#');
                        out.push_str(&name);
                    } else if name == "line" {
                        out.push_str(&line.to_string());
                    } else {
                        match self.resolve(&name) {
                            PreprocKind::Define(ReplaceVal::Str(value)) => out.push_str(value),
                            PreprocKind::DefineFn { params, body } => {
                                let (args, end) = parse_macro_args(&chars, j, &name);
                                out.push_str(&self.expand_fn_macro(
                                    &name,
                                    params,
                                    body,
                                    &args,
                                    line,
                                    depth + 1,
                                ));
                                j = end;
                            }
                        }
                    }
                    i = j;
                    continue;
                }
            }
            out.push(chars[i]);
            i += 1;
        }
        out
    }

    /// `#if #NAME` 用: panicせずに「定義済みかどうか」だけを調べる。
    pub(in crate::compiler::hexl_frontend::lex) fn is_defined(&self, name: &str) -> bool {
        self.tables.get(name).is_some()
    }

    /// プリプロセッサを処理し、展開済みのソースを作る。
    ///
    /// 対応する行頭ディレクティブ:
    /// - `#define NAME VALUE`: `Preprocessor` に登録する
    /// - `#define NAME {{ ... }}` / `#define NAME(args..) {{ ... }}`:
    ///   `{{`から最初の`}}`までを本体として、複数行にまたがって書ける
    /// - `#define NAME(arg, arg2, ..) BODY`: 関数形式マクロとして登録する。
    ///   `#NAME(a, b)`と書くと、`BODY`の仮引数を実引数に置き換えて展開する
    ///   (名前と`(`の間に空白を入れると、`NAME VALUE`の形式になる)
    /// - `#if #NAME` / `#elif #NAME` / `#else` / `#endif`: 上から順に条件を調べ、
    ///   最初に `NAME` が定義済みだった枝だけを有効にする(どれも当てはまらなければ
    ///   `#else` の枝)。`#elif`は何個でも、`#else`は省略可、入れ子可。
    ///   `#windows` / `#linux` は `--windows` / `--linux` で選んだOSだけが定義済みになる
    pub(in crate::compiler::hexl_frontend::lex) fn preprocess(&mut self, content: &str) -> Option<String> {
        Some(self.preprocess_text(content))
    }

    /// `preprocess` の本体(`#include` のインライン展開から再帰的にも呼ばれる)
    fn preprocess_text(&mut self, content: &str) -> String {
        /// `#if` / `#elif` / `#else` の入れ子1段分の状態。
        struct CondFrame {
            /// 外側のスコープ自体が有効かどうか
            parent_active: bool,
            /// 今いる枝(`#if` / `#elif` / `#else`)の条件が成り立っているか
            condition: bool,
            /// これまでの枝のどれかが既に採用されたか(以降の `#elif` / `#else` は無効)
            taken: bool,
            /// すでに `#else` を通過したか
            in_else: bool,
        }
        impl CondFrame {
            fn active(&self) -> bool {
                self.parent_active && self.condition
            }
        }
        fn is_active(stack: &[CondFrame]) -> bool {
            stack.last().map(CondFrame::active).unwrap_or(true)
        }

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
                while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '_') {
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
                                // `#if #A` のように、条件は `#NAME` の形で書く
                                let condition = self.is_defined(cond_target(&rest_of_line));
                                cond_stack.push(CondFrame {
                                    parent_active: active_here,
                                    condition,
                                    taken: condition,
                                    in_else: false,
                                });
                                i = line_end;
                                continue;
                            }
                            "elif" => {
                                let condition = self.is_defined(cond_target(&rest_of_line));
                                if let Some(frame) = cond_stack.last_mut() {
                                    if frame.in_else {
                                        panic!("`#elif` after `#else`");
                                    }
                                    // 既にどれかの枝を採用していたら、条件に関係なく無効
                                    frame.condition = !frame.taken && condition;
                                    frame.taken |= frame.condition;
                                }
                                i = line_end;
                                continue;
                            }
                            "else" => {
                                if let Some(frame) = cond_stack.last_mut() {
                                    frame.condition = !frame.taken;
                                    frame.taken = true;
                                    frame.in_else = true;
                                }
                                i = line_end;
                                continue;
                            }
                            "include" => {
                                // `#include "file"`(別名なし)だけは、ここでファイルを読み込み、
                                // その場にインライン展開する。`#include "file"::io` と
                                // `#include io="file"` は、これまでどおり後段に任せる。
                                if active_here {
                                    if let Some(rel) = plain_include_path(&rest_of_line) {
                                        let included = self.include_inline(&rel);
                                        output.push_str(&included);
                                        i = line_end;
                                        continue;
                                    }
                                }
                            }
                            "endif" => {
                                cond_stack.pop();
                                i = line_end;
                                continue;
                            }
                            _ => {
                                if active_here {
                                    if let Some((target, kind)) = sort_preproc(&name, &rest_of_line)
                                    {
                                        // `#define ...` 行: テーブルに登録
                                        self.add(target, kind);
                                        // 読み飛ばした改行は、後続の行番号がずれないよう
                                        // 定義の位置にそのまま出す
                                        for _ in 0..skipped_lines {
                                            output.push('\n');
                                        }
                                        current_line += skipped_lines;
                                        i = line_end;
                                        continue;
                                    }
                                } else if name == "define" && skipped_lines > 0 {
                                    // 無効な`#if`の中の複数行の定義は、まるごと読み飛ばす
                                    current_line += skipped_lines;
                                    i = line_end;
                                    continue;
                                }
                            }
                        }
                    }

                    // 行頭ディレクティブでなければ、インライン展開として扱う
                    // (`next`は、展開で読み飛ばした分を進めた後の位置)
                    let mut next = j;
                    if is_active(&cond_stack) {
                        if name == "asm" || name == "include" {
                            // `#asm(...)` は構文として残し、後段のパーサーが
                            // 直接解釈する。ここで未定義マクロ扱いにしてはならない。
                            output.push('#');
                            output.push_str(&name);
                        } else if name == "line" {
                            output.push_str(&current_line.to_string());
                        } else {
                            match self.resolve(&name) {
                                PreprocKind::Define(ReplaceVal::Str(value)) => {
                                    output.push_str(value);
                                }
                                PreprocKind::DefineFn { params, body } => {
                                    // `#NAME(a, b)`: `(`から対応する`)`までを実引数として読む
                                    let (args, end) = parse_macro_args(&chars, j, &name);
                                    output.push_str(&self.expand_fn_macro(
                                        &name,
                                        params,
                                        body,
                                        &args,
                                        current_line,
                                        0,
                                    ));
                                    // 呼び出しが複数行にまたがっていても、後続の行番号が
                                    // ずれないよう、読み飛ばした改行を展開結果の後ろへ出す
                                    let newlines =
                                        chars[j..end].iter().filter(|c| **c == '\n').count();
                                    for _ in 0..newlines {
                                        output.push('\n');
                                    }
                                    current_line += newlines;
                                    next = end;
                                }
                            }
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

/// `#` の直後に来た `Name` トークン(例: "define")を見て、
/// 改行までにスタックした残りの文字列をどう解釈するかを決める。
///
/// 仕様の手順:
/// 1. `#` を見つける
/// 2. 直後に `Name` が来たらフラグを立てる
/// 3. 改行が来るまで文字列をスタック
/// 5. 2で集めた名前を match で条件分岐する (命令が今後増える前提)
/// 6. 3の値と一緒にテーブルに置く
///
/// `name` が未知の命令なら `None` を返す。
pub(in crate::compiler::hexl_frontend::lex) fn sort_preproc<'a>(
    name: &str,
    rest_of_line: &'a str,
) -> Option<(&'a str, PreprocKind)> {
    match name {
        "define" => {
            // `NAME(a, b) BODY`: 名前の直後に`(`が続く場合は関数形式マクロ
            if let Some(defined) = parse_define_fn(rest_of_line.trim()) {
                return Some(defined);
            }
            // `A 10` のような形式を「登録する名前」と「値」に分ける
            let mut parts = rest_of_line.trim().splitn(2, char::is_whitespace);
            let target_name = parts.next()?;
            let value = parts.next().unwrap_or("").trim().to_string();
            Some((target_name, PreprocKind::Define(ReplaceVal::Str(value))))
        }
        _ => None,
    }
}

/// `#if #NAME` / `#elif #NAME` の条件部分(`#NAME`)から `NAME` を取り出す
fn cond_target(rest_of_line: &str) -> &str {
    rest_of_line.trim().trim_start_matches('#')
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

/// 識別子に使える文字か
fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// `NAME(arg, arg2, ..) BODY`(`#define`の後ろ)を、関数形式マクロとして読む。
/// 名前の直後(空白なし)に`(`が続かない場合は`None`(=従来の`NAME VALUE`)
///
/// 仮引数が識別子でない/重複している/`)`が無い場合は panic する。
fn parse_define_fn(text: &str) -> Option<(&str, PreprocKind)> {
    let name_end = text.find(|c: char| !is_name_char(c)).unwrap_or(text.len());
    if name_end == 0 || !text[name_end..].starts_with('(') {
        return None;
    }
    let name = &text[..name_end];
    let after_open = &text[name_end + 1..];
    let close = match after_open.find(')') {
        Some(close) => close,
        None => panic!("`#define {}(` is missing `)`", name),
    };

    let params_src = after_open[..close].trim();
    let mut params: Vec<String> = Vec::new();
    if !params_src.is_empty() {
        for param in params_src.split(',').map(str::trim) {
            if param.is_empty() || !param.chars().all(is_name_char) {
                panic!("invalid parameter `{}` in `#define {}(..)`", param, name);
            }
            if params.iter().any(|p| p == param) {
                panic!("duplicate parameter `{}` in `#define {}(..)`", param, name);
            }
            params.push(param.to_string());
        }
    }
    let body = after_open[close + 1..].trim().to_string();
    Some((name, PreprocKind::DefineFn { params, body }))
}

/// `#NAME(a, b)`の実引数を読む。`open`は`(`の位置(=`NAME`の直後)。
/// 戻り値は`(実引数のテキスト, 対応する`)`の次の位置)`
///
/// `,`で区切るのは括弧(`()`/`[]`/`{}`)の外側だけで、文字列リテラルの中は読み飛ばす。
/// `(`が無い/`)`で閉じていない場合は panic する。
fn parse_macro_args(chars: &[char], open: usize, name: &str) -> (Vec<String>, usize) {
    if chars.get(open) != Some(&'(') {
        panic!(
            "preprocessor macro `{}` requires arguments: `#{}(..)`",
            name, name
        );
    }
    let mut args: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut depth = 1usize;
    let mut k = open + 1;
    while k < chars.len() {
        let c = chars[k];
        match c {
            '"' => {
                // 文字列リテラルは閉じる`"`まで、そのまま実引数に含める
                cur.push(c);
                k += 1;
                while k < chars.len() {
                    let d = chars[k];
                    cur.push(d);
                    k += 1;
                    if d == '\\' && k < chars.len() {
                        cur.push(chars[k]);
                        k += 1;
                    } else if d == '"' {
                        break;
                    }
                }
                continue;
            }
            '(' | '[' | '{' => {
                depth += 1;
                cur.push(c);
            }
            ')' | ']' | '}' => {
                depth -= 1;
                if depth == 0 {
                    args.push(cur.trim().to_string());
                    return (args, k + 1);
                }
                cur.push(c);
            }
            ',' if depth == 1 => {
                args.push(cur.trim().to_string());
                cur.clear();
            }
            _ => cur.push(c),
        }
        k += 1;
    }
    panic!(
        "preprocessor macro `{}`: arguments are not closed with `)`",
        name
    );
}

/// マクロの本体`body`の中の仮引数(識別子)を、実引数のテキストに置き換える。
/// 文字列リテラルの中と、数値(`2x`など)の一部は置き換えない。
fn substitute_params(body: &str, params: &[String], args: &[String]) -> String {
    let chars: Vec<char> = body.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '"' {
            out.push(c);
            i += 1;
            while i < chars.len() {
                let d = chars[i];
                out.push(d);
                i += 1;
                if d == '\\' && i < chars.len() {
                    out.push(chars[i]);
                    i += 1;
                } else if d == '"' {
                    break;
                }
            }
        } else if is_name_char(c) {
            let start = i;
            while i < chars.len() && is_name_char(chars[i]) {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            // 数字で始まる語(`2x`など)は識別子ではないのでそのまま
            let replaced = if c.is_ascii_digit() {
                None
            } else {
                params.iter().position(|p| *p == word)
            };
            match replaced {
                Some(idx) => out.push_str(&args[idx]),
                None => out.push_str(&word),
            }
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

/// `#define`の見出し(`NAME` または `NAME(arg, ..)`)として正しい形か
fn is_define_header(head: &str) -> bool {
    if head.is_empty() {
        return false;
    }
    if head.ends_with(')') {
        match head.find('(') {
            Some(k) if k > 0 => head[..k].chars().all(is_name_char),
            _ => false,
        }
    } else {
        // 値のマクロの名前(空白を含まない)
        !head.chars().any(char::is_whitespace)
    }
}

/// `#define NAME {{ BODY }}` / `#define NAME(args..) {{ BODY }}` を読む。
///
/// `head_start`は`#define`の直後、`line_end`はその行の末尾の位置。
/// 見出しの後ろに`{{`が無い場合は`None`(=従来の単一行の`#define`)。
/// 戻り値は`(sort_preproc`へ渡す1行の定義`, `}}`がある行の末尾の位置)`。
/// 本体は、`{{`と最初の`}}`の間(前後の空白・改行は除く)。
///
/// `}}`で閉じていない、または`}}`の後ろに同じ行で何か書かれている場合は panic する。
fn read_multiline_define(
    chars: &[char],
    head_start: usize,
    line_end: usize,
) -> Option<(String, usize)> {
    let head: String = chars[head_start..line_end].iter().collect();
    let open = head.find("{{")?;
    let header = head[..open].trim();
    if !is_define_header(header) {
        return None;
    }

    let body_start = head_start + head[..open].chars().count() + 2;
    let mut close = body_start;
    while close + 1 < chars.len() && !(chars[close] == '}' && chars[close + 1] == '}') {
        close += 1;
    }
    if close + 1 >= chars.len() {
        panic!("`#define {}`: `{{{{` is not closed with `}}}}`", header);
    }
    let body: String = chars[body_start..close].iter().collect();

    // `}}`の後ろは、行末まで空白だけ
    let mut end = close + 2;
    while end < chars.len() && chars[end] != '\n' {
        if !chars[end].is_whitespace() {
            panic!("`#define {}`: unexpected text after `}}}}`", header);
        }
        end += 1;
    }
    Some((format!("{} {}", header, body.trim()), end))
}
