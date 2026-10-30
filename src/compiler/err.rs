use crate::compiler::err::compiler_err::CompilerErr;
use crate::compiler::hexl_frontend::lex;
use std::fmt;
use std::rc::Rc;

pub mod lex_err;
pub mod opt;
pub mod syntax_err;
pub mod undef;
pub use compiler_err::*;
pub use syntax_err::*;
pub mod compile;
pub use compile::*;
pub mod compiler_err;
pub mod err_pool;

// ---------------------------------------------------------------------
// 位置情報
// --------------------------------------------------------------------

/// 解析対象のソースコード上の位置(何行目の何文字目か)
#[derive(Debug, Clone, PartialEq, Eq, Default, Hash)]
pub struct Span {
    pub line: usize,
    pub pos: usize,
    pub line_txt: String,
    pub self_file_name: Rc<String>,
}

impl Span {
    pub fn new(
        line: usize,
        pos: usize,
        line_txt: String,
        self_file_name: &Rc<String>,
    ) -> Self {
        Self {
            line: line,
            pos: pos,
            line_txt,
            self_file_name: Rc::clone(&self_file_name),
        }
    }

    /// 位置情報が存在しない(取得できない)場合に使う
    #[cfg(not(test))]
    pub fn unknown() -> Self {
        Self::new(
            0,
            0,
            "undefine".to_string(),
            &Rc::new(UNKNOWN_FILE_NAME.to_string()),
        )
    }

    /// 位置情報が存在しない(取得できない)場合に使う
    #[cfg(test)]
    pub fn unknown() -> Self {
        Self::new(0, 0, "undefine".to_string(), &Rc::new("test".to_string()))
    }

    /// `Span::unknown()`で作られた(位置が分からない)か
    pub fn is_unknown(&self) -> bool {
        self.line == 0 && self.pos == 0
    }

    /// このSpanが指すソースファイルの名前
    #[cfg(not(test))]
    pub fn file_name(&self) -> &str {
        self.self_file_name.as_str()
    }

    /// このSpanが指すソースファイルの名前(テストでは固定の`<test>`)
    #[cfg(test)]
    pub fn file_name(&self) -> &str {
        TEST_FILE_NAME
    }

    /// `ファイル名:行:文字` 形式の位置(Rustの`-->`の後ろと同じ形式)
    pub fn location(&self) -> String {
        format!("{}:{}:{}", self.file_name(), self.line, self.pos)
    }
}

/// 位置が分からないときのファイル名
pub const UNKNOWN_FILE_NAME: &str = "<unknown>";
/// テスト(`cfg(test)`)では`Span`がファイル名を持たないので、代わりにこれを返す
#[cfg(test)]
pub const TEST_FILE_NAME: &str = "<test>";

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}の{}行目 {}文字目",
            self.file_name(),
            self.line,
            self.pos
        )
    }
}

// ---------------------------------------------------------------------
// エラー表示の共通部品
//
// すべてのエラーの`Display`は `{種別}: {メッセージ} [{位置}]` に統一する。
// 位置が無い(または`Span::unknown()`の)ときは ` [{位置}]` を省く。
// ---------------------------------------------------------------------

pub const CAT_SYNTAX: &str = "構文エラー";
pub const CAT_PREPROC: &str = "プリプロセッサエラー";
pub const CAT_LEX: &str = "字句解析エラー";
pub const CAT_COMPILE: &str = "コンパイルエラー";
pub const CAT_UNDEF: &str = "未定義エラー";
pub const CAT_INTERNAL: &str = "内部エラー";
pub const CAT_OPT: &str = "オプションエラー";

/// 位置が分かっているときだけ`Some`を返す
pub(crate) fn span_loc(span: Option<&Rc<Span>>) -> Option<&Span> {
    span.map(|s| &**s).filter(|s| !s.is_unknown())
}

/// `{種別}: {メッセージ} [{位置}]` を書き出す
pub(crate) fn write_err(
    f: &mut fmt::Formatter<'_>,
    category: &str,
    msg: &dyn fmt::Display,
    loc: Option<&dyn fmt::Display>,
) -> fmt::Result {
    match loc {
        Some(loc) => write!(f, "{}: {} [{}]", category, msg, loc),
        None => write!(f, "{}: {}", category, msg),
    }
}

/// エラーが「どこで(`Span`)」「どの関数で」発生したかをまとめた情報。
///
/// すべてのエラーの種類([`SyntaxErrKind`]・[`CondErrKinds`]・
/// [`PreprocErrs`])は、必ずこの`ErrLoc`とセットで保持される。
#[derive(Debug, Clone, PartialEq)]
pub struct ErrLoc {
    /// 解析していたソースコード上の位置
    pub span: Rc<Span>,
    /// エラーを発生させた(パーサー側の)関数の名前
    ///
    /// [`func_name!`]マクロによって、モジュールパス付きの
    /// フルパス(例: `parse::expr::cond::expr_match`)が入る。
    pub func_name: String,
}

impl ErrLoc {
    pub fn new(span: Rc<Span>, func_name: String) -> Self {
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

#[derive(Debug, Clone, PartialEq)]
pub struct ErrAt {
    pub error: CompileErr,
    pub span: Rc<Span>,
}

/// `parse`クレート全体で使う、集約されたエラー型。
#[derive(Debug, Clone, PartialEq)]
pub enum ErrKind {
    /// 見つかるべきトークンが見つからなかった(簡易版)
    NotFoundTkn(Box<lex::Tkn>),
    /// 予期しないトークンだった(簡易版、詳細情報なし)
    UnexpectedToken,
    CompileErr(Box<CompileErr>),
    CompileErrAt(Box<ErrAt>),
    CompilerErr(Box<CompilerErr>),
    /// トークン管理・式解析など、構文解析全般のエラー
    Syntax(Box<SyntaxErr>),
    Undef(Box<undef::UndefErrs>),
    /// プリプロセッサ特有のエラー
    Preproc(Box<PreprocErrDetail>),
}

impl fmt::Display for ErrKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFoundTkn(t) => {
                write_err(f, CAT_SYNTAX, &format!("`{:?}`が見つかりません", t), None)
            }
            Self::UnexpectedToken => write_err(f, CAT_SYNTAX, &"予期しないトークンです", None),
            Self::Syntax(e) => write!(f, "{}", e),
            Self::Preproc(e) => write!(f, "{}", e),
            Self::CompileErr(e) => write!(f, "{}", e),
            Self::CompileErrAt(e) => write_err(
                f,
                CAT_COMPILE,
                &e.error.kind.title(),
                span_loc(Some(&e.span)).map(|s| s as &dyn fmt::Display),
            ),
            Self::Undef(e) => write!(f, "{}", e),
            Self::CompilerErr(e) => write!(f, "{}", e),
        }
    }
}

impl ErrKind {
    pub fn with_span(self, span: Rc<Span>) -> Self {
        match self {
            Self::CompileErr(error) => Self::CompileErrAt(Box::new(ErrAt {
                error: *error,
                span: span,
            })),
            // 位置が未設定のエラーには、ここで位置(=ファイル名を含む)を付ける
            Self::Undef(mut e) => {
                if e.span.is_none() {
                    e.span = Some(span);
                }
                Self::Undef(e)
            }
            Self::CompilerErr(mut e) => {
                if e.span.is_none() {
                    e.span = Some(span);
                }
                Self::CompilerErr(e)
            }
            other => other,
        }
    }

    /// エラーの発生位置(持っていないエラーは`None`)
    pub fn span(&self) -> Option<&Rc<Span>> {
        match self {
            Self::Syntax(e) => Some(&e.span),
            Self::CompileErr(e) => Some(&e.span),
            Self::CompileErrAt(e) => Some(&e.span),
            Self::Preproc(e) => Some(&e.loc.span),
            Self::Undef(e) => e.span.as_ref(),
            Self::CompilerErr(e) => e.span.as_ref(),
            Self::NotFoundTkn(_) | Self::UnexpectedToken => None,
        }
    }

    /// エラーが発生したソースファイルの名前(位置が分からないときは`None`)
    pub fn file_name(&self) -> Option<&str> {
        span_loc(self.span()).map(|s| s.file_name())
    }
}

impl std::error::Error for ErrKind {}

impl From<SyntaxErr> for ErrKind {
    fn from(e: SyntaxErr) -> Self {
        ErrKind::Syntax(Box::new(e))
    }
}

impl From<PreprocErrDetail> for ErrKind {
    fn from(e: PreprocErrDetail) -> Self {
        ErrKind::Preproc(Box::new(e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_includes_position_and_func_name() {
        let span = Rc::new(Span::new(3, 10, String::new(), &Rc::new("test".to_string())));
        let e = ErrKind::Syntax(Box::new(SyntaxErr {
            kind: SyntaxErrKind::TknIsEofInExpr,
            loc: ErrLoc::new(span.clone(), "expr_value".to_string()),
            span,
        }));
        let msg = e.to_string();
        assert!(msg.starts_with("構文エラー: "));
        assert!(msg.contains("3行目"));
        assert!(msg.contains("10文字目"));
        assert!(msg.contains("expr_value"));
        // ファイル名も表示される(テストでは`<test>`)
        assert!(msg.contains("<test>"));
        assert_eq!(e.file_name(), Some("<test>"));
    }

    #[test]
    fn undef_err_gets_span_from_with_span() {
        let e = undef::UndefErrs::undef_fn("foo");
        assert_eq!(e.to_string(), "未定義エラー: 関数`foo`が見つかりません");
        let e = e.with_span(Rc::new(Span::new(2, 5, String::new(), &Rc::new("test".to_string()))));
        let msg = e.to_string();
        assert!(msg.starts_with("未定義エラー: 関数`foo`が見つかりません ["));
        assert!(msg.contains("<test>"));
        assert!(msg.contains("2行目"));
    }

    #[test]
    fn compile_err_display_has_file_name() {
        let e = ErrKind::CompileErr(Box::new(CompileErr::new(
            CompileErrKind::BreakOutsideLoop,
            Rc::new(Span::new(7, 1, String::new(), &Rc::new("test".to_string()))),
        )));
        let msg = e.to_string();
        assert!(msg.starts_with("コンパイルエラー: "));
        assert!(msg.contains("<test>"));
        assert!(msg.contains("7行目"));
    }

    #[test]
    fn unknown_span_has_no_location() {
        let e = ErrKind::CompileErr(Box::new(CompileErr::new(
            CompileErrKind::BreakOutsideLoop,
            Rc::new(Span::unknown()),
        )));
        assert!(!e.to_string().contains('['));
        assert_eq!(e.file_name(), None);
    }

    #[test]
    fn cond_err_kind_message() {
        let kind = CondErrKinds::CondElseNotFound;
        assert_eq!(kind.to_string(), "`cond`式に`|`(else)が見つかりません");
    }
}