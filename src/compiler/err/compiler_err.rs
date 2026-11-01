//! 構文ではなくこのhexlコンパイラ自身のエラー

use super::{CAT_INTERNAL, Span, span_loc, write_err};
use crate::compiler::ir::types;
use std::fmt;
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq)]
pub enum CompilerErrKind {
    ThisTyIsCantConvertRegsiter(types::Size),
    /// 正規表現を作れなかった(中身は正規表現のエラーの文)
    RegexBuildFailed(String),
    /// コンパイラの内部で、想定していない種類のノードが来た
    ///
    /// 呼び出し側が事前に種類を確認しているはずなので、本来は起こらない
    UnexpectedNodeKind {
        /// どの処理で起きたか(関数名など)
        place: &'static str,
        /// 来るはずだったノード
        expected: &'static str,
        /// 実際に来たノード(デバッグ表示)
        found: String,
    },
    /// `Segments`形式のパスにしか使えない処理を、`Literal`形式のパスに使った
    LiteralImportPathNotSupported {
        /// どの処理で起きたか(関数名)
        place: &'static str,
    },
}

impl CompilerErrKind {
    /// エラーコード(`E09xx`は、コンパイラ内部のエラー)
    pub fn code(&self) -> &'static str {
        match self {
            Self::ThisTyIsCantConvertRegsiter(..) => "E0900",
            Self::RegexBuildFailed(..) => "E0901",
            Self::UnexpectedNodeKind { .. } => "E0902",
            Self::LiteralImportPathNotSupported { .. } => "E0903",
        }
    }
}

impl fmt::Display for CompilerErrKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ThisTyIsCantConvertRegsiter(size) => {
                write!(f, "サイズ`{:?}`の型はレジスタに変換できません", size)
            }
            Self::RegexBuildFailed(detail) => {
                write!(f, "正規表現を作れませんでした: {}", detail)
            }
            Self::UnexpectedNodeKind {
                place,
                expected,
                found,
            } => write!(
                f,
                "`{}`で想定していない種類のノードです(期待: {} / 実際: {})",
                place, expected, found
            ),
            Self::LiteralImportPathNotSupported { place } => write!(
                f,
                "`{}`は、文字列リテラルの`#include`のパスには使えません",
                place
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompilerErr {
    pub kind: CompilerErrKind,
    /// エラーの発生位置。分からない(コンパイラ内部で起きた)ときは`None`
    pub span: Option<Rc<Span>>,
}

impl fmt::Display for CompilerErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_err(
            f,
            CAT_INTERNAL,
            &self.kind,
            span_loc(self.span.as_ref()).map(|s| s as &dyn fmt::Display),
        )
    }
}

impl CompilerErr {
    /// `kind`と位置から、`ErrKind`を作る(`Err(..)`で包まない版)
    ///
    /// 位置が分からないときは`None`を渡す(後から`ErrKind::with_span`で付けられる)。
    /// `DebugMode`のときは、ここで`panic!`する
    #[track_caller]
    pub fn at(kind: CompilerErrKind, span: Option<Rc<Span>>) -> super::ErrKind {
        super::raise(super::ErrKind::CompilerErr(Box::new(Self { kind, span })))
    }
}

/// `GenCompilerErr!(kind)` / `GenCompilerErr!(kind, span)`
#[macro_export]
macro_rules! GenCompilerErr {
    ($kind:expr) => {
        Err($crate::compiler::err::raise(err::ErrKind::CompilerErr(Box::new(CompilerErr {
            kind: $kind,
            span: None,
        }))))
    };
    ($kind:expr, $span:expr) => {
        Err($crate::compiler::err::raise(err::ErrKind::CompilerErr(Box::new(CompilerErr {
            kind: $kind,
            span: Some($span),
        }))))
    };
}
