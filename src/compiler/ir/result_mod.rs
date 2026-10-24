//! エラーの扱い方を型で切り替える`ResultMod`
//!
//! `gen_expr_ir::<M>`のように、`M: ResultMod`を指定した関数は、中で起きた
//! エラー(`Result<T, ErrKind>`の`Err`)を`M::handle`に通す。
//!
//! | 型 | `Err`のとき |
//! |---|---|
//! | [`ReturnErr`] | そのまま`Err`として返す(呼び出し側が`?`で伝播する) |
//! | [`PanicErr`] | その場で`panic!`する(`Err`は返らない) |
//!
//! 戻り値はどちらも`Result`なので、関数の型は変わらない。
//! `PanicErr`では`Err`が返らないので、`.unwrap()`しても安全。
//! `src/ir/builder.rs`の`mod result_mod;`で読み込む。

use crate::compiler::err;

/// `Result`の`Err`の扱い方
pub(crate) trait ResultMod {
    /// `result`が`Err`のときの扱いを決める
    fn handle<T>(result: Result<T, err::ErrKind>) -> Result<T, err::ErrKind>;
}

/// `Err`をそのまま返す(エラーを呼び出し側へ伝える)
///
/// 型引数(`builder::<ReturnErr>`)としてだけ使う。テスト以外の呼び出しは
/// `PanicErr`なので、通常のビルドでは構築されない警告を出さない
#[allow(dead_code)]
pub struct ReturnErr;

impl ResultMod for ReturnErr {
    #[inline(always)]
    fn handle<T>(result: Result<T, err::ErrKind>) -> Result<T, err::ErrKind> {
        result
    }
}

/// `Err`ならその場で`panic!`する
pub struct PanicErr;

impl ResultMod for PanicErr {
    #[inline(always)]
    #[track_caller]
    fn handle<T>(result: Result<T, err::ErrKind>) -> Result<T, err::ErrKind> {
        match result {
            Ok(v) => Ok(v),
            Err(e) => panic!("{:?}", e),
        }
    }
}
