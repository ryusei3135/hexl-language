//! プリプロセッサ(`preproc.rs`)、inlineアセンブラ(`asm_expr.rs`)で発生するエラー

use super::*;

impl Parser {
    /// `#include`などのパスの要素(名前/文字列/`*`)が来るはずの位置に、別のトークンがあった
    pub(in crate::compiler::parse)
    fn expected_path_segment<T>(&self) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::preproc_err!(self, ExpectedPathSegment)
    }

    /// `#asm(name)`の`name`が無かった
    pub(in crate::compiler::parse)
    fn not_found_asm_name<T>(&self) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::preproc_err!(self, NotFoundAsmName)
    }

    /// `#asm`の次に`(`が無かった
    pub(in crate::compiler::parse)
    fn expected_lparen_after_asm<T>(&self) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::preproc_err!(self, ExpectedLParenAfterAsm)
    }

    /// `#asm(name`の次に`)`が無かった
    pub(in crate::compiler::parse)
    fn expected_rparen_after_asm<T>(&self) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::preproc_err!(self, ExpectedRParenAfterAsm)
    }

    /// プリプロセッサの構文として解釈できないトークンがあった
    pub(in crate::compiler::parse)
    fn preproc_unexpected_token<T>(&self) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        Err(err::ErrKind::UnexpectedToken)
    }

    /// `${}`の中が空だった
    pub(in crate::compiler::parse)
    fn empty_asm_operand<T>() -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        Err(crate::preproc_err_at!(
            err::Span::new(0, 0),
            EmptyAsmOperand
        ))
    }

    /// `${...}`の式の解析が終わった後も、トークンが残っていた
    pub(in crate::compiler::parse)
    fn unexpected_trailing_tkn_in_asm_operand<T>(&self) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::preproc_err!(self, UnexpectedTrailingTokenInAsmOperand)
    }

    /// `${[p}`のように、`]`で閉じられていなかった
    pub(in crate::compiler::parse)
    fn expected_rbracket_in_asm_operand<T>(&self) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::preproc_err!(self, ExpectedRBracketInAsmOperand)
    }

    /// `${(a}`のように、`)`で閉じられていなかった
    pub(in crate::compiler::parse)
    fn expected_rparen_in_asm_operand<T>(&self) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::preproc_err!(self, ExpectedRParenInAsmOperand)
    }

    /// `${}`の中に、式として解釈できないトークンがあった
    pub(in crate::compiler::parse)
    fn unexpected_tkn_in_asm_operand<T>(&self) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::preproc_err!(self, UnexpectedTokenInAsmOperand)
    }

    /// `${x.}`のように、`.`の次がメンバーの名前ではなかった
    pub(in crate::compiler::parse)
    fn expected_member_name_in_asm_operand<T>(&self) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::preproc_err!(self, ExpectedMemberNameInAsmOperand)
    }
}
