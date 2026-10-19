//! エラーを集めて、Rustのコンパイラのような形式で表示する
//!
//! ```text
//! error: `;`が必要ですが、`}`が見つかりました
//!  --> main.src:3:9
//!   |
//! 3 |     a: int = }
//!   |              ^ ここに`}`があります
//!   |
//!   = help: 式が途中で終わっていないか確認してください
//!
//! error: aborting due to 1 previous error
//! ```
//!
//! エラー箇所のソースコードを表示するには、先に[`ErrPool::add_source`]で
//! ファイル名(`SyntaxErr::file_name`と同じ文字列)とソースコードを登録する。
//! 登録されていないファイルは、位置(`file:line:pos`)だけを表示する。

use crate::compiler::err;

use super::*;

use std::collections::HashMap;
use std::io::IsTerminal;

/// 行番号を1始まりとして扱う(`Span::line`が0始まりなら`0`にする)
const LINE_BASE: usize = 1;
/// 列(`Span::pos`)を1始まりとして扱う(0始まりなら`0`にする)
const POS_BASE: usize = 1;
/// タブを表示するときの幅
const TAB_WIDTH: usize = 4;

#[derive(Debug, PartialEq)]
pub struct ErrPool {
    err_list: Vec<ErrKind>,
    /// ファイル名 -> ソースコード
    sources: HashMap<String, String>,
}

impl ErrPool {
    pub fn new() -> Self {
        Self {
            err_list: Vec::new(),
            sources: HashMap::new(),
        }
    }

    pub fn push_err_node(&mut self, err_node: ErrKind) {
        self.err_list.push(err_node);
    }

    /// エラー箇所の表示に使うソースコードを登録する
    ///
    /// `file_name`は、`SyntaxErr::file_name`と同じ文字列にする
    pub fn add_source(&mut self, file_name: &str, source: &str) {
        self.sources
            .insert(file_name.to_string(), source.to_string());
    }

    /// 集めたエラーを標準エラー出力に表示する
    ///
    /// 端末に出力するときだけ色を付ける(`NO_COLOR`が設定されていれば付けない)
    pub fn view(&self) {
        let color = std::io::stderr().is_terminal() && std::env::var_os("NO_COLOR").is_none();
        eprint!("{}", self.render(color));
    }

    /// 表示する文字列を作る(`color`が`true`ならANSIの色付き)
    pub fn render(&self, color: bool) -> String {
        let mut out = String::new();
        let mut count = 0;
        for e in self.err_list.iter() {
            match e {
                ErrKind::Syntax(n) => {
                    out.push_str(&self.render_syntax_err(n, color));
                    out.push('\n');
                    count += 1;
                }
                _ => {}
            }
        }
        if count > 0 {
            let msg = if count == 1 {
                "aborting due to 1 previous error".to_string()
            } else {
                format!("aborting due to {} previous errors", count)
            };
            out.push_str(&format!(
                "{}: {}\n",
                paint("error", RED_BOLD, color),
                bold(&msg, color)
            ));
        }
        out
    }

    fn render_syntax_err(&self, n: &err::SyntaxErr, color: bool) -> String {
        let line = n.loc.span.line as usize;
        let pos = n.loc.span.pos as usize;
        let file = n.file_name.as_str();

        let mut out = String::new();
        // error: メッセージ
        out.push_str(&format!(
            "{}{}\n",
            paint("error", RED_BOLD, color),
            bold(&format!(": {}", n.kind), color)
        ));

        let line_no = line.saturating_sub(LINE_BASE) + 1; // 表示する行番号(1始まり)
        let col_no = pos.saturating_sub(POS_BASE) + 1; // 表示する列(1始まり)
        let width = line_no.to_string().len();
        let pad = " ".repeat(width);
        let bar = paint("|", BLUE_BOLD, color);

        // --> file:line:col
        out.push_str(&format!(
            "{}{} {}:{}:{}\n",
            pad,
            paint("-->", BLUE_BOLD, color),
            file,
            line_no,
            col_no
        ));

        let label = Self::label(&n.kind);
        let help = Self::help(&n.kind);

        match self.source_line(file, line_no) {
            Some(src_line) => {
                let (shown, caret_col) = expand_line(src_line, col_no - 1);
                out.push_str(&format!("{} {}\n", pad, bar));
                out.push_str(&format!(
                    "{} {} {}\n",
                    paint(
                        &format!("{:>width$}", line_no, width = width),
                        BLUE_BOLD,
                        color
                    ),
                    bar,
                    shown
                ));
                out.push_str(&format!(
                    "{} {} {}{}{}\n",
                    pad,
                    bar,
                    " ".repeat(caret_col),
                    paint("^", RED_BOLD, color),
                    if label.is_empty() {
                        String::new()
                    } else {
                        format!(" {}", paint(&label, RED_BOLD, color))
                    }
                ));
                if help.is_some() {
                    out.push_str(&format!("{} {}\n", pad, bar));
                }
            }
            None => {
                // ソースコードが無い場合は、ラベルだけ表示する
                if !label.is_empty() {
                    out.push_str(&format!(
                        "{} {} {}\n",
                        pad,
                        paint("=", BLUE_BOLD, color),
                        format!("{}: {}", bold("note", color), label)
                    ));
                }
            }
        }
        if let Some(help) = help {
            out.push_str(&format!(
                "{} {} {}: {}\n",
                pad,
                paint("=", BLUE_BOLD, color),
                bold("help", color),
                help
            ));
        }
        out
    }

    /// 登録されたソースコードの`line_no`行目(1始まり)
    fn source_line(&self, file: &str, line_no: usize) -> Option<&str> {
        self.sources.get(file)?.lines().nth(line_no.checked_sub(1)?)
    }

    /// `^`の横に表示する短い説明
    fn label(kind: &err::SyntaxErrKind) -> String {
        use err::SyntaxErrKind as K;
        match kind {
            K::TknIsEof { expected } => {
                format!("ここで終了しています(期待: {})", expected.join(" / "))
            }
            K::UnexpectedTkn {
                found, expected, ..
            } => {
                format!("`{:?}`が必要ですが、`{:?}`があります", expected, found)
            }
            K::OptionIsNotFound { found, .. } => format!("`{}`は使えません", found),
            K::UnexpectTknAfterKeyword {
                keyword,
                expected,
                found,
            } => format!(
                "`{:?}`の後には{}が必要ですが、`{:?}`があります",
                keyword,
                expected.join(" / "),
                found
            ),
            K::UnenclosedScope { .. } => "ここまでに`}`が見つかりませんでした".to_string(),
            K::NotFoundTkn(t) => format!("`{:?}`が必要です", t),
            K::UnexpectTknInExpr { found } => format!("`{:?}`は式の中で使えません", found),
            K::TknIsEofInExpr => "式の途中でファイルが終わっています".to_string(),
            K::Cond(kind) => kind.to_string(),
            K::UnexpectTknInStmt { found } => format!("`{:?}`は文の先頭に置けません", found),
            K::ExpectedKind { expected, found } => {
                format!("{}が必要ですが、`{:?}`があります", expected, found)
            }
            K::NotImplemented { feature } => format!("`{}`は未対応です", feature),
            K::MissingEqualsAfterMust { found } => {
                format!("`=`が必要ですが、`{:?}`があります", found)
            }
            K::MissingIdentAfterOf { found } => {
                format!("名前が必要ですが、`{:?}`があります", found)
            }
            K::GenericsNotSupportedYet => "ジェネリクスは未対応です".to_string(),
        }
    }

    /// `= help:`に表示する、直し方のヒント
    fn help(kind: &err::SyntaxErrKind) -> Option<&'static str> {
        use err::SyntaxErrKind as K;
        match kind {
            K::TknIsEof { .. } | K::TknIsEofInExpr => {
                Some("式や文が途中で終わっていないか、括弧が閉じられているか確認してください")
            }
            K::UnenclosedScope { .. } => Some("対応する`{`に合わせて、`}`を追加してください"),
            K::UnexpectTknInExpr { .. } => {
                Some("演算子やオペランドが抜けていないか確認してください")
            }
            K::UnexpectTknInStmt { .. } => Some("前の文が正しく終わっているか確認してください"),
            K::Cond(_) => Some("`cond`は`cond { 条件 { .. } | { .. } }`の形で書きます"),
            K::NotImplemented { .. } | K::GenericsNotSupportedYet => {
                Some("この機能は今のコンパイラでは使えません")
            }
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------
// 色付け
// ---------------------------------------------------------------------

const RED_BOLD: &str = "1;31";
const BLUE_BOLD: &str = "1;34";

fn paint(text: &str, code: &str, color: bool) -> String {
    if color {
        format!("\x1b[{}m{}\x1b[0m", code, text)
    } else {
        text.to_string()
    }
}

fn bold(text: &str, color: bool) -> String {
    paint(text, "1", color)
}

// ---------------------------------------------------------------------
// 表示用の補助
// ---------------------------------------------------------------------

/// 端末での表示幅(ASCIIは1、それ以外は全角として2)
fn char_width(c: char) -> usize {
    match c {
        '\t' => TAB_WIDTH,
        c if c.is_ascii() => 1,
        // 半角カナ
        '\u{ff61}'..='\u{ff9f}' => 1,
        _ => 2,
    }
}

/// 行の表示用の文字列(タブを空白に展開)と、`char_idx`文字目の表示上の列を返す
fn expand_line(line: &str, char_idx: usize) -> (String, usize) {
    let mut shown = String::new();
    let mut col = 0;
    let mut caret = None;
    for (i, c) in line.chars().enumerate() {
        if i == char_idx {
            caret = Some(col);
        }
        if c == '\t' {
            shown.push_str(&" ".repeat(TAB_WIDTH));
        } else {
            shown.push(c);
        }
        col += char_width(c);
    }
    // 行末(例えばEOF)を指しているときは、行の末尾に`^`を置く
    (shown, caret.unwrap_or(col))
}
