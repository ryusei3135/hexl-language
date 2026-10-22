//! トークンを次に進めたり、次のトークンの参照
//! などのトークンにアクセスするAPIを提供
//! するモジュール
use std::rc::Rc;

use super::*;
use stmt::*;

impl Parser {
    /// 現在の位置から1つ先のトークンを取得する。
    pub(in crate::compiler::hexl_frontend::parse) fn peek_tkn(&self) -> Result<lex::Tkn, err::ErrKind> {
        if let Some(tkn) = &self.tkns {
            if let Some(r) = tkn.get(self.idx + 1).map(|v| v.tkn.clone()) {
                Ok(r)
            } else {
                crate::err_at!(self.tkn_is_eof(Vec::new()))
            }
        } else {
            crate::err_at!(self.tkn_is_eof(Vec::new()))
        }
    }

    /// `peek_tkn`の2つ先版。`#include`の
    /// `Name "=" ...`(エイリアス指定)のように、2つ先まで
    /// 覗き見てからでないと構文を判断できない場合に使う
    pub(in crate::compiler::hexl_frontend::parse) fn peek2_tkn(&self) -> Option<lex::Tkn> {
        self.tkns.as_ref()?.get(self.idx + 2).map(|v| v.tkn.clone())
    }

    /// `peek_tkn`の3つ先版。キャスト`(T)var`のように、
    /// `(` `Name` `)` `Name`という並びかを、位置を進めずに判断するときに使う
    pub(in crate::compiler::hexl_frontend::parse) fn peek3_tkn(&self) -> Option<lex::Tkn> {
        self.tkns.as_ref()?.get(self.idx + 3).map(|v| v.tkn.clone())
    }

    /// 次のトークンが存在する場合だけ位置を1つ進める。
    /// 存在しない場合は位置を変えずに`None`を返す
    pub(in crate::compiler::hexl_frontend::parse) fn advance_tkn(&mut self) -> Option<lex::Tkn> {
        if let Some(tkns) = &self.tkns {
            if let Some(next) = tkns.get(self.idx + 1) {
                self.idx += 1;
                return Some(next.tkn.clone());
            }
        }
        None
    }

    /// `advance_tkn`の、次のトークンが無い場合に`Err`(EOF)を返す版。
    /// 直前の`peek_tkn`などで次のトークンの存在を確認済みの箇所で、
    /// `advance_tkn().unwrap()`の代わりに使う
    pub(in crate::compiler::hexl_frontend::parse) fn advance_tkn_checked(
        &mut self,
    ) -> Result<lex::Tkn, err::ErrKind> {
        match self.advance_tkn() {
            Some(tkn) => Ok(tkn),
            None => crate::err_at!(self.tkn_is_eof(Vec::new())),
        }
    }

    /// 位置を1つ戻す。
    /// `,`を省略できる構文(構造体/列挙型のメンバー区切りなど)で
    /// 直後の`next_tkn`呼び出しが現在のトークンをもう一度
    /// 読み直せるようにするために使う。
    pub(in crate::compiler::hexl_frontend::parse) fn back_tkn(&mut self) {
        self.idx -= 1;
    }
    /// なにのトークンが期待されていたかは呼び出し元で決める
    pub(in crate::compiler::hexl_frontend::parse) fn next_tkn(
        &mut self,
        expected: &[&'static str],
    ) -> Result<lex::Tkn, err::ErrKind> {
        self.idx += 1;

        if let Some(tkns) = &self.tkns {
            if let Some(val) = tkns.get(self.idx) {
                return Ok(val.tkn.clone());
            }
        }
        crate::err_at!(self.tkn_is_eof(expected.to_vec()))
    }

    /// なにのトークンが期待されていたかは呼び出し元で決める
    pub(in crate::compiler::hexl_frontend::parse) fn next_tkn_ref(
        &self,
        expected: &[&'static str],
    ) -> Result<lex::Tkn, err::ErrKind> {
        if let Some(tkns) = &self.tkns {
            if let Some(val) = tkns.get(self.idx + 1) {
                return Ok(val.tkn.clone());
            }
        }
        crate::err_at!(self.tkn_is_eof(expected.to_vec()))
    }

    /// エラーが発生したときのどの行の何文字目がエラーかを
    /// 確認する構造体を作成する
    pub(in crate::compiler::hexl_frontend::parse) fn build_err_span(&self) -> Rc<err::Span> {
        // 範囲外はエラーの位置を表示するための値なので`0`を返す
        if let Some(span) = self.tkns.as_ref().and_then(|tkns| tkns.get(self.idx)) {
            Rc::clone(&span.span)
        } else {
            if let Some(span) = self.tkns.as_ref().and_then(|x| x.get(self.idx - 1)) {
                Rc::clone(&span.span)
            } else {
                panic!();
            }
        }
    }

    #[inline(always)]
    pub(in crate::compiler::hexl_frontend::parse) fn current_tkn(&self) -> &lex::Tkn {
        &self.tkns.as_ref().unwrap()[self.idx].tkn
    }

    #[inline(always)]
    pub(in crate::compiler::hexl_frontend::parse) fn current_line(&self, idx: usize) -> usize {
        // 範囲外はエラーの位置を表示するための値なので`0`を返す
        self.tkns
            .as_ref()
            .and_then(|tkns| tkns.get(idx))
            .map_or(0, |t| t.span.line)
    }

    #[inline(always)]
    pub(in crate::compiler::hexl_frontend::parse) fn tkn_chr_pos(&self) -> usize {
        self.tkns
            .as_ref()
            .and_then(|tkns| tkns.get(self.idx.saturating_sub(1)))
            .map_or(0, |t| t.span.pos)
    }
}
