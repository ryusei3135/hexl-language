//! プリプロセッサ(`preproc.rs`)、inlineアセンブラ(`asm_expr.rs`)で発生するエラー

use super::*;

impl Parser {
    /// `#include`などのパスの要素(名前/文字列/`*`)が来るはずの位置に、別のトークンがあった
    pub(in crate::compiler::hexl_frontend::parse) fn expected_path_segment<T>(
        &self,
    ) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::preproc_err!(
            self.build_err_span(),
            ExpectedPathSegment,
            Rc::clone(&self.file_name)
        )
    }

    /// `#asm(name)`の`name`が無かった
    pub(in crate::compiler::hexl_frontend::parse) fn not_found_asm_name<T>(
        &self,
    ) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::preproc_err!(
            self.build_err_span(),
            NotFoundAsmName,
            Rc::clone(&self.file_name)
        )
    }

    /// `#asm`の次に`(`が無かった
    pub(in crate::compiler::hexl_frontend::parse) fn expected_lparen_after_asm<T>(
        &self,
    ) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::preproc_err!(
            self.build_err_span(),
            ExpectedLParenAfterAsm,
            Rc::clone(&self.file_name)
        )
    }

    /// `#asm(name`の次に`)`が無かった
    pub(in crate::compiler::hexl_frontend::parse) fn expected_rparen_after_asm<T>(
        &self,
    ) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::preproc_err!(
            self.build_err_span(),
            ExpectedRParenAfterAsm,
            Rc::clone(&self.file_name)
        )
    }

    /// プリプロセッサの構文として解釈できないトークンがあった
    pub(in crate::compiler::hexl_frontend::parse) fn preproc_unexpected_token<T>(
        &self,
    ) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        Err(err::ErrKind::UnexpectedToken)
    }

    /// `${}`の中が空だった
    pub(in crate::compiler::hexl_frontend::parse) fn empty_asm_operand<T>(
        &self,
    ) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        Err(crate::preproc_err_at!(
            self.build_err_span(),
            EmptyAsmOperand
        ))
    }

    /// `${...}`の式の解析が終わった後も、トークンが残っていた
    pub(in crate::compiler::hexl_frontend::parse) fn unexpected_trailing_tkn_in_asm_operand<T>(
        &self,
    ) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::preproc_err!(
            self.build_err_span(),
            UnexpectedTrailingTokenInAsmOperand,
            Rc::clone(&self.file_name)
        )
    }

    /// `${[p}`のように、`]`で閉じられていなかった
    pub(in crate::compiler::hexl_frontend::parse) fn expected_rbracket_in_asm_operand<T>(
        &self,
    ) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::preproc_err!(
            self.build_err_span(),
            ExpectedRBracketInAsmOperand,
            Rc::clone(&self.file_name)
        )
    }

    /// `${(a}`のように、`)`で閉じられていなかった
    pub(in crate::compiler::hexl_frontend::parse) fn expected_rparen_in_asm_operand<T>(
        &self,
    ) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::preproc_err!(
            self.build_err_span(),
            ExpectedRParenInAsmOperand,
            Rc::clone(&self.file_name)
        )
    }

    /// `${}`の中に、式として解釈できないトークンがあった
    pub(in crate::compiler::hexl_frontend::parse) fn unexpected_tkn_in_asm_operand<T>(
        &self,
    ) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::preproc_err!(
            self.build_err_span(),
            UnexpectedTokenInAsmOperand,
            Rc::clone(&self.file_name)
        )
    }

    /// `${x.}`のように、`.`の次がメンバーの名前ではなかった
    pub(in crate::compiler::hexl_frontend::parse) fn expected_member_name_in_asm_operand<T>(
        &self,
    ) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::preproc_err!(
            self.build_err_span(),
            ExpectedMemberNameInAsmOperand,
            Rc::clone(&self.file_name)
        )
    }
}
