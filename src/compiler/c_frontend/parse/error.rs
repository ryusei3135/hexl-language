//! パーサのエラー型

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseErrorKind {
    /// Cの文法として正しくない
    Syntax,
    /// Cとしては正しいが、`node`で表現する方法が無い(NOTE.md参照)
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub kind: ParseErrorKind,
    pub line: usize,
    pub col: usize,
    pub msg: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let k = match self.kind {
            ParseErrorKind::Syntax => "構文エラー",
            ParseErrorKind::Unsupported => "nodeで未対応",
        };
        write!(f, "{}:{}: {}: {}", self.line, self.col, k, self.msg)
    }
}

impl std::error::Error for ParseError {}

pub type PResult<T> = Result<T, ParseError>;

impl ParseError {
    pub fn syntax(line: usize, col: usize, msg: impl Into<String>) -> Self {
        Self {
            kind: ParseErrorKind::Syntax,
            line,
            col,
            msg: msg.into(),
        }
    }
    pub fn unsupported(line: usize, col: usize, msg: impl Into<String>) -> Self {
        Self {
            kind: ParseErrorKind::Unsupported,
            line,
            col,
            msg: msg.into(),
        }
    }
}
