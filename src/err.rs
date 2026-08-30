use crate::lex;
use std::fmt;

pub mod syntax_err;
pub mod opt;
pub mod undef;
pub mod lex_err;
pub use syntax_err::*;

// ---------------------------------------------------------------------
// 位置情報
// ---------------------------------------------------------------------

/// 解析対象のソースコード上の位置(何行目の何文字目か)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub line: usize,
    pub pos: usize,
}

impl Span {
    pub fn new(line: &usize, pos: &usize) -> Self {
        Self {
            line: *line,
            pos: *pos,
        }
    }

    /// 位置情報が存在しない(取得できない)場合に使う
    pub fn unknown() -> Self {
        Self { line: 0, pos: 0 }
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}行目 {}文字目", self.line, self.pos)
    }
}

/// エラーが「どこで(`Span`)」「どの関数で」発生したかをまとめた情報。
///
/// すべてのエラーの種類([`SyntaxErrKind`]・[`CondErrKinds`]・
/// [`PreprocErrs`])は、必ずこの`ErrLoc`とセットで保持される。
#[derive(Debug, Clone, PartialEq)]
pub struct ErrLoc {
    /// 解析していたソースコード上の位置
    pub span: Span,
    /// エラーを発生させた(パーサー側の)関数の名前
    ///
    /// [`func_name!`]マクロによって、モジュールパス付きの
    /// フルパス(例: `parse::expr::cond::expr_match`)が入る。
    pub func_name: String,
}

impl ErrLoc {
    pub fn new(span: Span, func_name: String) -> Self {
        Self { span, func_name }
    }
}

impl fmt::Display for ErrLoc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({}関数)", self.span, self.func_name)
    }
}

/// 呼び出された場所を囲む関数の名前を、モジュールパス付きで取得するマクロ。
///
/// Rustの標準機能だけでは「今実行中の関数名」を直接取得できないため、
/// ローカル関数`f`を定義し、その`type_name`から関数名を逆算するという
/// 定番のテクニックを使っている。
#[macro_export]
macro_rules! func_name {
    () => {{
        fn f() {}
        fn type_name_of<T>(_: T) -> &'static str {
            std::any::type_name::<T>()
        }
        let name = type_name_of(f);
        // 末尾の"::f"を取り除き、関数名だけを残す
        name.strip_suffix("::f").unwrap_or(name).to_string()
    }};
}


// ---------------------------------------------------------------------
// 集約されたエラー
// ---------------------------------------------------------------------

/// `parse`クレート全体で使う、集約されたエラー型。
#[derive(Debug, Clone, PartialEq)]
pub enum ErrKind {
    /// 見つかるべきトークンが見つからなかった(簡易版)
    NotFoundTkn(lex::Tkn),
    /// 予期しないトークンだった(簡易版、詳細情報なし)
    UnexpectedToken,
    /// トークン管理・式解析など、構文解析全般のエラー
    Syntax(SyntaxErr),
    /// プリプロセッサ特有のエラー
    Preproc(PreprocErrDetail),
}

impl fmt::Display for ErrKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFoundTkn(t) => write!(f, "`{:?}`が見つかりません", t),
            Self::UnexpectedToken => write!(f, "予期しないトークンです"),
            Self::Syntax(e) => write!(f, "{}", e),
            Self::Preproc(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for ErrKind {}

impl From<SyntaxErr> for ErrKind {
    fn from(e: SyntaxErr) -> Self {
        ErrKind::Syntax(e)
    }
}

impl From<PreprocErrDetail> for ErrKind {
    fn from(e: PreprocErrDetail) -> Self {
        ErrKind::Preproc(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_includes_position_and_func_name() {
        let e = ErrKind::Syntax(SyntaxErr {
            kind: SyntaxErrKind::TknIsEofInExpr,
            loc: ErrLoc::new(Span::new(&3, &10), "expr_value".to_string()),
        });
        let msg = e.to_string();
        assert!(msg.contains("3行目"));
        assert!(msg.contains("10文字目"));
        assert!(msg.contains("expr_value"));
    }

    #[test]
    fn cond_err_kind_message() {
        let kind = CondErrKinds::CondElseNotFound;
        assert_eq!(kind.to_string(), "`cond`式に`|`(else)が見つかりません");
    }
}
