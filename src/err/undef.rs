
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

#[derive(Debug, Clone)]
pub enum UndefKind {
    UndefVarTy,
}

#[derive(Debug, Clone)]
pub struct UndefErrs {
    kind: UndefKind,
    found: String,
    expect: String,
}
