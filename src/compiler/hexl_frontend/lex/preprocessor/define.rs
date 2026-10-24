//! `#define`(値のマクロ・関数形式マクロ)の読み取りと展開

use super::include::is_kept_directive;
use super::{is_name_char, line, PreprocKind, Preprocessor, ReplaceVal};

/// 関数形式マクロの展開の入れ子の上限(`#define F(a) #F(a)`のような自己再帰の防止)
const MAX_EXPAND_DEPTH: usize = 64;

impl Preprocessor {
    /// 関数形式マクロ`#name(args..)`を展開する。
    ///
    /// 1. 実引数の中の`#NAME`を先に展開する
    /// 2. `body`の中の仮引数を実引数のテキストに置き換える(文字列リテラルの中は置き換えない)
    /// 3. 置き換えた結果の中の`#NAME`(他のマクロの呼び出し)を展開する
    ///
    /// 実引数の数が仮引数の数と違う場合、入れ子が深すぎる場合は panic する。
    pub(super) fn expand_fn_macro(
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
    pub(super) fn expand_inline(&self, text: &str, line: usize, depth: usize) -> String {
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
                    if is_kept_directive(&name) {
                        out.push('#');
                        out.push_str(&name);
                    } else if name == line::NAME {
                        out.push_str(&line::line_text(line));
                    } else {
                        match self.resolve(&name) {
                            PreprocKind::Define(ReplaceVal::Str(value)) => out.push_str(value),
                            PreprocKind::DefineFn {
                                params,
                                body,
                                brace,
                            } => {
                                let (args, end) = parse_macro_args(&chars, j, &name, *brace);
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

    /// `#define`の行(`#define`の直後からの文字列`rest_of_line`)を処理する。
    ///
    /// - 有効な枝: マクロを登録する。読み飛ばした改行は、後続の行番号がずれないよう
    ///   定義の位置にそのまま出す
    /// - 無効な`#if`の枝: 複数行の定義(`skipped_lines > 0`)は、まるごと読み飛ばす
    ///
    /// この行をここで処理し終えたら`true`(呼び出し側は`i = line_end`へ進める)
    pub(super) fn define_directive(
        &mut self,
        rest_of_line: &str,
        active: bool,
        skipped_lines: usize,
        output: &mut String,
        current_line: &mut usize,
    ) -> bool {
        if active {
            if let Some((target, kind)) = sort_preproc("define", rest_of_line) {
                // `#define ...` 行: テーブルに登録
                self.add(target, kind);
                for _ in 0..skipped_lines {
                    output.push('\n');
                }
                *current_line += skipped_lines;
                return true;
            }
        } else if skipped_lines > 0 {
            // 無効な`#if`の中の複数行の定義は、まるごと読み飛ばす
            *current_line += skipped_lines;
            return true;
        }
        false
    }

    /// 行の途中などにある`#NAME`/`#NAME(args..)`/`#NAME { args.. }`を展開して`output`へ出す。
    /// `j`は`NAME`の直後の位置で、戻り値は展開で読み飛ばした分を進めた後の位置。
    pub(super) fn expand_call(
        &self,
        name: &str,
        chars: &[char],
        j: usize,
        output: &mut String,
        current_line: &mut usize,
    ) -> usize {
        match self.resolve(name) {
            PreprocKind::Define(ReplaceVal::Str(value)) => {
                output.push_str(value);
                j
            }
            PreprocKind::DefineFn {
                params,
                body,
                brace,
            } => {
                // `#NAME(a, b)` / `#NAME { a, b }`:
                // 開き括弧から対応する閉じ括弧までを実引数として読む
                let (args, end) = parse_macro_args(chars, j, name, *brace);
                output.push_str(&self.expand_fn_macro(name, params, body, &args, *current_line, 0));
                // 呼び出しが複数行にまたがっていても、後続の行番号が
                // ずれないよう、読み飛ばした改行を展開結果の後ろへ出す
                let newlines = chars[j..end].iter().filter(|c| **c == '\n').count();
                for _ in 0..newlines {
                    output.push('\n');
                }
                *current_line += newlines;
                end
            }
        }
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

/// `NAME(arg, arg2, ..) BODY` / `NAME{arg, arg2, ..} BODY`(`#define`の後ろ)を、
/// 関数形式マクロとして読む。
/// 名前の直後(空白なし)に`(`か`{`が続かない場合は`None`(=従来の`NAME VALUE`)。
/// `NAME{{`は複数行の値のマクロの書き出しなので、ここでは扱わない
///
/// 仮引数が識別子でない/重複している/閉じ括弧が無い場合は panic する。
fn parse_define_fn(text: &str) -> Option<(&str, PreprocKind)> {
    let name_end = text.find(|c: char| !is_name_char(c)).unwrap_or(text.len());
    if name_end == 0 {
        return None;
    }
    let (brace, close_ch) = match &text[name_end..] {
        rest if rest.starts_with("{{") => return None,
        rest if rest.starts_with('(') => (false, ')'),
        rest if rest.starts_with('{') => (true, '}'),
        _ => return None,
    };
    let name = &text[..name_end];
    let after_open = &text[name_end + 1..];
    let close = match after_open.find(close_ch) {
        Some(close) => close,
        None => panic!(
            "`#define {}{}` is missing `{}`",
            name,
            if brace { '{' } else { '(' },
            close_ch
        ),
    };

    let params_src = after_open[..close].trim();
    let mut params: Vec<String> = Vec::new();
    if !params_src.is_empty() {
        for param in params_src.split(',').map(str::trim) {
            if param.is_empty() || !param.chars().all(is_name_char) {
                panic!("invalid parameter `{}` in `#define {}`", param, name);
            }
            if params.iter().any(|p| p == param) {
                panic!("duplicate parameter `{}` in `#define {}`", param, name);
            }
            params.push(param.to_string());
        }
    }
    let body = after_open[close + 1..].trim().to_string();
    Some((
        name,
        PreprocKind::DefineFn {
            params,
            body,
            brace,
        },
    ))
}

/// `#NAME(a, b)` / `#NAME { a, b }`の実引数を読む。`open`は`NAME`の直後の位置。
/// 戻り値は`(実引数のテキスト, 対応する閉じ括弧の次の位置)`
///
/// `brace`が`false`なら`(`から`)`まで、`true`なら`{`から`}`まで(名前と`{`の間の
/// 空白・タブは読み飛ばす)。`,`で区切るのは括弧(`()`/`[]`/`{}`)の外側だけで、
/// 文字列リテラルの中は読み飛ばす。
/// 開き括弧が無い/対応する閉じ括弧で閉じていない場合は panic する。
fn parse_macro_args(chars: &[char], open: usize, name: &str, brace: bool) -> (Vec<String>, usize) {
    let (open_ch, close_ch) = if brace { ('{', '}') } else { ('(', ')') };
    let mut open = open;
    if brace {
        while matches!(chars.get(open), Some(' ') | Some('\t')) {
            open += 1;
        }
    }
    if chars.get(open) != Some(&open_ch) {
        panic!(
            "preprocessor macro `{}` requires arguments: `#{} {}..{}`",
            name, name, open_ch, close_ch
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
                    if c != close_ch {
                        panic!(
                            "preprocessor macro `{}`: arguments must be closed with `{}`",
                            name, close_ch
                        );
                    }
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
        "preprocessor macro `{}`: arguments are not closed with `{}`",
        name, close_ch
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

/// `#define`の見出し(`NAME` / `NAME(arg, ..)` / `NAME{arg, ..}`)として正しい形か
fn is_define_header(head: &str) -> bool {
    if head.is_empty() {
        return false;
    }
    if head.ends_with(')') || head.ends_with('}') {
        let open_ch = if head.ends_with(')') { '(' } else { '{' };
        match head.find(open_ch) {
            Some(k) if k > 0 => head[..k].chars().all(is_name_char),
            _ => false,
        }
    } else {
        // 値のマクロの名前(空白を含まない)
        !head.chars().any(char::is_whitespace)
    }
}

/// `#define NAME {{ BODY }}` / `#define NAME(args..) {{ BODY }}` /
/// `#define NAME{args..} {{ BODY }}` を読む。
///
/// `head_start`は`#define`の直後、`line_end`はその行の末尾の位置。
/// 見出しの後ろに`{{`が無い場合は`None`(=従来の単一行の`#define`)。
/// 戻り値は`(sort_preproc`へ渡す1行の定義`, `}}`がある行の末尾の位置)`。
/// 本体は、`{{`と最初の`}}`の間(前後の空白・改行は除く)。
///
/// `}}`で閉じていない、または`}}`の後ろに同じ行で何か書かれている場合は panic する。
pub(super) fn read_multiline_define(
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
