//! トークンを次に進めたり、次のトークンの参照
//! などのトークンにアクセスするAPIを提供
//! するモジュール

use super::*;
use stmt::*;

impl Parser {
    /// 現在の位置から1つ先のトークンを取得する。
    pub(super) fn peek_tkn(
        &self
    ) -> Result<lex::Tkn, err::ErrKind> {
        if let Some(tkn) = &self.tkns {
            if let Some(r) = tkn.get(self.idx + 1)
                .map(|v| v.tkn.clone()) {
                    Ok(r)
            } else {
                crate::syntax_err!(
                    self.build_err_span(), 
                    err::SyntaxErrKind::TknIsEof { 
                        expected: Vec::new(),
                    }
                )
            }
        } else {
            crate::syntax_err!(
                self.build_err_span(), 
                err::SyntaxErrKind::TknIsEof { 
                    expected: Vec::new(),
                }
            )
        }
    }

    /// `peek_tkn`の2つ先版。`#include`の
    /// `Name "=" ...`(エイリアス指定)のように、2つ先まで
    /// 覗き見てからでないと構文を判断できない場合に使う
    pub(super) fn peek2_tkn(&self) -> Option<lex::Tkn> {
        self.tkns
            .as_ref()
            .unwrap()
            .get(self.idx + 2)
            .map(|v| v.tkn.clone())
    }

    /// 次のトークンが存在する場合だけ位置を1つ進める。
    /// 存在しない場合は位置を変えずに`None`を返す
    pub(super) fn advance_tkn(
        &mut self
    ) -> Option<lex::Tkn> {
        if let Some(tkns) = &self.tkns {
            if let Some(next) = tkns.get(self.idx + 1) {
                self.idx += 1;
                return Some(next.tkn.clone());
            }
        }
        None
    }

    /// 位置を1つ戻す。
    /// `,`を省略できる構文(構造体/列挙型のメンバー区切りなど)で
    /// 直後の`next_tkn`呼び出しが現在のトークンをもう一度
    /// 読み直せるようにするために使う。
    pub(super) fn back_tkn(&mut self) {
        self.idx -= 1;
    }
    /// なにのトークンが期待されていたかは呼び出し元で決める
    pub(super) fn next_tkn(
        &mut self,
        expected: &[&'static str],
    ) -> Result<lex::Tkn, err::ErrKind> {
        self.idx += 1;

        if let Some(tkns) = &self.tkns {
            if let Some(val) = tkns.get(self.idx) {
                return Ok(val.tkn.clone())
            }
        }
        crate::syntax_err!(
            self.build_err_span(), 
            err::SyntaxErrKind::TknIsEof { 
                expected: expected.to_vec()
            }
        )
    }

    /// なにのトークンが期待されていたかは呼び出し元で決める
    pub(super) fn next_tkn_ref(
        &self,
        expected: &[&'static str],
    ) -> Result<lex::Tkn, err::ErrKind> {
        if let Some(tkns) = &self.tkns {
            if let Some(val) = tkns.get(self.idx + 1) {
                return Ok(val.tkn.clone())
            }
        }
        crate::syntax_err!(
            self.build_err_span(), 
            err::SyntaxErrKind::TknIsEof { 
                expected: expected.to_vec()
            }
        )
    }

    /// エラーが発生したときのどの行の何文字目がエラーかを
    /// 確認する構造体を作成する
    pub(super) fn build_err_span(&self) -> err::Span {
        err::Span::new(
            self.current_line(self.idx.saturating_sub(1)), 
            self.tkn_chr_pos()
        )
    }

    #[inline(always)]
    pub(super) fn current_tkn(&self) -> &lex::Tkn {
        &self.tkns.as_ref().unwrap()[self.idx].tkn
    }

    #[inline(always)]
    pub(super) fn current_line(&self, idx: usize) -> usize {
        self.tkns.as_ref().unwrap()[idx].line
    }

    #[inline(always)]
    pub(super) fn tkn_chr_pos(&self) -> usize {
        self.tkns.as_ref().unwrap()[self.idx.saturating_sub(1)].pos
    }
}