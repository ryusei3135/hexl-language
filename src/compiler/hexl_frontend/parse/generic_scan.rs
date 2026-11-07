//! ジェネリクスの定義(`name<T, U>(..) { .. }` / `struct Name<T, U> { .. }`)を
//! トークン列から切り出すための共通処理
//!
//! 関数(`func.rs`)と構造体/バリアント(`typedef/generic.rs`)の
//! 定義の切り出しで、同じ処理が重複していたため、ここに集約している。
//! エラーはどちらも`generic_def_expected`/`generic_def_eof`が作る。

use super::*;

impl Parser {
    /// `tkns[lt_idx]`の`<`から始まる型パラメータ`<T, U>`を読む。
    ///
    /// 型パラメータ同士で名前が重複してはいけない。`reserved`が`Some`なら、
    /// その名前も型パラメータに使えない(型自身の名前を指定する)。
    ///
    /// ## 戻り値
    /// `(型パラメータ, 閉じる`>`のインデックス)`
    pub(super) fn read_generic_params(
        &self,
        tkns: &[lex::LocatedTkn],
        lt_idx: usize,
        reserved: Option<&str>,
    ) -> Result<(Vec<String>, usize), err::ErrKind> {
        let expected = |i: usize, expected: &'static str| {
            crate::err_at!(self.generic_def_expected(i, expected))
        };
        let eof = |expected: Vec<&'static str>| crate::err_at!(self.generic_def_eof(expected));

        let mut params = Vec::<String>::new();
        // `<`を指している
        let mut i = lt_idx;
        loop {
            i += 1;
            match tkns.get(i).map(|t| &t.tkn) {
                Some(lex::Tkn::Name(param)) => {
                    if params.contains(param) || reserved == Some(param.as_str()) {
                        return expected(i, "unique type parameter name");
                    }
                    params.push(param.clone());
                }
                Some(_) => return expected(i, "name"),
                None => return eof(vec!["name"]),
            }
            i += 1;
            match tkns.get(i).map(|t| &t.tkn) {
                Some(lex::Tkn::Comma) => continue,
                Some(lex::Tkn::RAngleBracket) => return Ok((params, i)),
                Some(_) => return expected(i, ", or `>`"),
                None => return eof(vec![",", ">"]),
            }
        }
    }

    /// `tkns[body_start]`の`{`から始まる本体を閉じる、`}`のインデックスを返す
    /// (`{ .. }`の入れ子も数える)。閉じていなければトークン終端のエラー
    pub(super) fn find_block_end(
        &self,
        tkns: &[lex::LocatedTkn],
        body_start: usize,
    ) -> Result<usize, err::ErrKind> {
        let mut depth = 0usize;
        let mut end = body_start;
        loop {
            match tkns.get(end).map(|t| &t.tkn) {
                Some(lex::Tkn::LBrace) => depth += 1,
                Some(lex::Tkn::RBrace) => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(end);
                    }
                }
                Some(_) => {}
                None => return crate::err_at!(self.generic_def_eof(vec!["}"])),
            }
            end += 1;
        }
    }
}

/// 型引数`<..>`の先頭(`<`の次)になれるトークンか。
/// 違えば型引数ではない(`f<>(..)`や`f<3>(..)`など)
pub(super) fn is_type_args_start(tkn: Option<&lex::Tkn>) -> bool {
    matches!(
        tkn,
        Some(
            lex::Tkn::Name(_)
                | lex::Tkn::LBracket
                | lex::Tkn::KeyWordStatic
                | lex::Tkn::KeyWordSelf
        )
    )
}

/// 型引数の中に現れる、型を構成するトークンか(`<`・`>`は含まない)
pub(super) fn is_type_component(tkn: &lex::Tkn) -> bool {
    matches!(
        tkn,
        lex::Tkn::Name(_)
            | lex::Tkn::Comma
            | lex::Tkn::Mul
            | lex::Tkn::KeyWordConst
            | lex::Tkn::KeyWordMut
            | lex::Tkn::KeyWordStatic
            | lex::Tkn::KeyWordSelf
            | lex::Tkn::LBracket
            | lex::Tkn::RBracket
            | lex::Tkn::Number(_)
    )
}
