//! トークンを次に進めたり、次のトークンの参照
//! などのトークンにアクセスするAPIを提供
//! するモジュール

use super::*;
use stmt::*;

impl Parser {
    /// 現在の位置から1つ先のトークンを取得する。
    /// `next_tkn_ref`と違い、それ以上トークンが無い場合は
    /// エラーではなく`None`を返す。
    ///
    /// inlineアセンブラの`${...}`内の式は、文の途中ではなく
    /// それだけで閉じた短いトークン列として解析されるため、
    /// 最後まで解析した後に続くトークンが存在しないことがある。
    /// そのため通常の文の解析(常に後続のトークンがある前提)
    /// とは違い、EOFをエラーにしない先読みが必要になる。
    pub(super) fn peek_tkn(&self) -> Option<lex::Tkn> {
        self.tkns
            .as_ref()
            .unwrap()
            .get(self.idx + 1)
            .map(|v| v.tkn.clone())
    }

    /// 次のトークンが存在する場合だけ位置を1つ進める。
    /// 存在しない場合は位置を変えずに`None`を返す
    pub(super) fn advance_tkn(&mut self) -> Option<lex::Tkn> {
        if let Some(next) = self.tkns.as_ref().unwrap().get(self.idx + 1) {
            self.idx += 1;
            Some(next.tkn.clone())
        } else {
            None
        }
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
        expected: Vec<&'static str>,
    ) -> Result<lex::Tkn, err::ErrKind> {
        self.idx += 1;
        if let Some(value) = self.tkns.as_ref().unwrap().get(self.idx) {
            Ok(value.tkn.clone())
        } else {
            err::SyntaxErr::tkn_is_eof(self.build_err_span(), expected)
        }
    }

    /// なにのトークンが期待されていたかは呼び出し元で決める
    pub(super) fn next_tkn_ref(
        &self,
        expected: Vec<&'static str>,
    ) -> Result<lex::Tkn, err::ErrKind> {
        if let Some(value) = self.tkns.as_ref().unwrap().get(self.idx + 1) {
            Ok(value.tkn.clone())
        } else {
            err::SyntaxErr::tkn_is_eof(self.build_err_span(), expected)
        }
    }

    /// エラーが発生したときのどの行の何文字目がエラーかを
    /// 確認する構造体を作成する
    pub(super) fn build_err_span(&self) -> err::Span {
        err::Span::new(self.current_line(self.idx - 1), self.tkn_chr_pos())
    }

    #[inline(always)]
    pub(super) fn current_tkn(&self) -> &lex::Tkn {
        &self.tkns.as_ref().unwrap()[self.idx].tkn
    }

    #[inline(always)]
    pub(super) fn current_line(&self, idx: usize) -> &usize {
        &self.tkns.as_ref().unwrap()[idx].line
    }

    #[inline(always)]
    pub(super) fn tkn_chr_pos(&self) -> &usize {
        &self.tkns.as_ref().unwrap()[self.idx - 1].pos
    }
}
