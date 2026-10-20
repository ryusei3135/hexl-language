//! # 未定義で使うエラー型
//!
//! ## 公開
//!
//! - [`UndefKind`]  : 未定義の種類
//! - [`UndefErrs`] : 未定義のエラー
//!
//! ## エラーの組み立て方
//! 各`kind`を直接組み立てて`ErrLoc`を手で埋めるのは面倒なので、
//! 代わりに次のマクロを使う。マクロの中で[`func_name!`]を展開する
//! ことで、「そのマクロを実際に書いた場所を囲む関数の名前」を
//! 自動的に取得できる。
//!
//! - [`cond_err!`]        : `CondErrKinds`から`ErrKind`を組み立てる
//! - [`preproc_err!`]     : `Parser`(または`build_err_span`を持つ値)から
//!                          `PreprocErrs`の`ErrKind`を作り、その場で`return`する
//! - [`preproc_err_at!`]  : 位置(`Span`)を直接指定して`PreprocErrs`の
//!                          `ErrKind`を作る(式として使う)

use crate::compiler::err;

#[derive(Debug, Clone, PartialEq)]
pub enum UndefKind {
    UndefVarTy,
    UndefVar,
    UndefFunc,
    UndefFuncRetTy,
    UndefStruct,
    UndefStructMember,
    UndefMemberInVar,
    UndefMemberInFn,
    UndefVariant,
    UndefVariantField,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UndefErrs {
    pub kind: UndefKind,
    pub found: String,
    pub expect: Option<String>,
}

impl UndefErrs {
    pub fn undef_fn(found: &str) -> err::ErrKind {
        err::ErrKind::Undef(Box::new(Self {
            kind: UndefKind::UndefFunc,
            found: found.to_owned(),
            expect: None,
        }))
    }
}

impl UndefKind {
    /// エラーコード(`E01xx`形式。`CompileErrKind`の`E00xx`とは別系統)
    pub fn code(&self) -> &'static str {
        match self {
            Self::UndefVarTy => "E0101",
            Self::UndefVar => "E0102",
            Self::UndefFunc => "E0103",
            Self::UndefFuncRetTy => "E0104",
            Self::UndefStruct => "E0105",
            Self::UndefStructMember => "E0106",
            Self::UndefMemberInVar => "E0107",
            Self::UndefMemberInFn => "E0108",
            Self::UndefVariant => "E0109",
            Self::UndefVariantField => "E0110",
        }
    }

    /// 何が見つからなかったかを表す名詞(`cannot find ...`の部分)
    fn what(&self) -> &'static str {
        match self {
            Self::UndefVarTy => "型",
            Self::UndefVar => "変数",
            Self::UndefFunc => "関数",
            Self::UndefFuncRetTy => "関数の戻り値の型",
            Self::UndefStruct => "構造体",
            Self::UndefStructMember => "構造体のメンバー",
            Self::UndefMemberInVar => "変数のメンバー",
            Self::UndefMemberInFn => "関数のメンバー",
            Self::UndefVariant => "バリアント型",
            Self::UndefVariantField => "バリアントのフィールド",
        }
    }

    /// `expect`が「メンバーの持ち主の名前」を表す種類か
    /// (それ以外の種類では、`expect`は「代わりの候補」として扱う)
    fn expect_is_owner(&self) -> bool {
        matches!(
            self,
            Self::UndefStructMember
                | Self::UndefMemberInVar
                | Self::UndefMemberInFn
                | Self::UndefVariantField
        )
    }

    /// 直し方のヒント
    fn help(&self) -> &'static str {
        match self {
            Self::UndefVarTy | Self::UndefFuncRetTy => {
                "型の綴りを確認するか、構造体/バリアント型を先に定義してください"
            }
            Self::UndefVar => "変数を使う前に定義しているか、スコープの外で使っていないか確認してください",
            Self::UndefFunc => "関数名の綴りを確認するか、呼び出す前に関数を定義してください",
            Self::UndefStruct => "構造体の名前の綴りを確認するか、先に構造体を定義してください",
            Self::UndefVariant => "バリアント型の名前の綴りを確認するか、先に定義してください",
            Self::UndefStructMember
            | Self::UndefMemberInVar
            | Self::UndefMemberInFn
            | Self::UndefVariantField => "メンバー名の綴りと、定義されているメンバーの一覧を確認してください",
        }
    }
}

impl UndefErrs {
    /// 見出し(`error[E0103]: `の後ろに出る文)
    pub fn title(&self) -> String {
        let what = self.kind.what();
        match (&self.expect, self.kind.expect_is_owner()) {
            (Some(owner), true) => {
                format!("`{owner}`に{what}`{}`が見つかりません", self.found)
            }
            _ => format!("{what}`{}`が見つかりません", self.found),
        }
    }

    /// `= note:`に出す補足
    pub fn note(&self) -> Option<String> {
        match (&self.expect, self.kind.expect_is_owner()) {
            (Some(cand), false) => Some(format!("もしかして: `{cand}`")),
            _ => Some(format!("`{}`はこの位置から見える範囲で定義されていません", self.found)),
        }
    }

    /// `= help:`に出す直し方
    pub fn help(&self) -> String {
        self.kind.help().to_string()
    }

    pub fn code(&self) -> &'static str {
        self.kind.code()
    }
}

#[macro_export]
macro_rules! GenUndefErrResult {
    ($kind:ident, $found:expr, $expect:expr) => {
        err::ErrKind::Undef(Box::new(crate::compiler::err::undef::UndefErrs {
            kind: $kind,
            found: $found,
            expect: $expect,
        }))
    };
}