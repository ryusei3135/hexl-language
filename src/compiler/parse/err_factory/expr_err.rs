//! 式(`expr.rs`、`expr/`、`path.rs`)で発生するエラー

use super::*;

impl Parser {
    /// 変数の定義の`=`が来るはずの位置などに、予期しないトークンがあった
    pub(in crate::compiler::parse)
    fn unexpect_tkn_in_expr<T>(&self) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::UnexpectTknInExpr {
                found: self.current_tkn().clone(),
            }
        )
    }

    /// 式の途中でトークンが終わった
    pub(in crate::compiler::parse)
    fn tkn_is_eof_in_expr<T>(&self) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::syntax_err!(self.build_err_span(), err::SyntaxErrKind::TknIsEofInExpr)
    }

    /// `[name].member`の`.`の次に、メンバーの名前が来なかった
    pub(in crate::compiler::parse)
    fn ptr_member_name_not_found<T>(&self, found: lex::Tkn) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::ExpectedKind {
                expected: "name",
                found,
            }
        )
    }

    // ===== 配列へのアクセス(`expr/arr_access.rs`) =====

    /// `[`の直後に、さらに`[`が来た(アドレスの取得として解釈できない)
    pub(in crate::compiler::parse)
    fn arr_access_unexpected_lbracket<T>(&self) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        Err(err::ErrKind::UnexpectedToken)
    }

    /// `name.[member index]`の`member`が名前ではなかった
    pub(in crate::compiler::parse)
    fn arr_member_name_not_found<T>(&self, found: lex::Tkn) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::ExpectedKind {
                expected: "name",
                found,
            }
        )
    }

    /// `name.[member index]`の`index`が数字ではなかった
    pub(in crate::compiler::parse)
    fn arr_member_index_not_number<T>(&self, found: lex::Tkn) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::ExpectedKind {
                expected: "number",
                found,
            }
        )
    }

    /// `[name index] = value`の`=`が無かった
    pub(in crate::compiler::parse)
    fn arr_assign_equal_not_found<T>(&self, found: lex::Tkn) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::ExpectedKind {
                expected: "=",
                found,
            }
        )
    }

    /// `[name index else ..]`の`index`の次が、`else`でも`]`でもなかった
    pub(in crate::compiler::parse)
    fn arr_else_or_rbracket_not_found<T>(&self, found: lex::Tkn) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::ExpectedKind {
                expected: "`else` or `]`",
                found,
            }
        )
    }

    /// `[name index else ..]`が`]`で閉じられていなかった
    pub(in crate::compiler::parse)
    fn arr_rbracket_not_found<T>(&self) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::ExpectedKind {
                expected: "]",
                found: self.current_tkn().clone(),
            }
        )
    }

    // ===== 範囲付きポインタ(`expr/range.rs`) =====

    /// 範囲付きポインタの`*`が無かった
    pub(in crate::compiler::parse)
    fn range_ptr_mul_not_found<T>(&self) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        Err(err::ErrKind::UnexpectedToken)
    }

    /// `*`の次が、数字でも`const`/`mut`でもなかった
    pub(in crate::compiler::parse)
    fn range_ptr_start_unexpected<T>(&self, found: lex::Tkn) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::ExpectedKind {
                expected: "number or `const`/`mut`",
                found,
            }
        )
    }

    /// 範囲の始まりの数字が無かった
    pub(in crate::compiler::parse)
    fn range_ptr_start_not_number<T>(&self) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        Err(err::ErrKind::UnexpectedToken)
    }

    /// 範囲の`..`が無かった
    pub(in crate::compiler::parse)
    fn range_ptr_range_tkn_not_found<T>(&self) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        Err(err::ErrKind::UnexpectedToken)
    }

    /// 範囲の終わりが数字ではなかった
    pub(in crate::compiler::parse)
    fn range_ptr_end_not_number<T>(&self, found: lex::Tkn) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::ExpectedKind {
                expected: "number",
                found,
            }
        )
    }

    /// 範囲付きポインタが`]`で閉じられていなかった
    pub(in crate::compiler::parse)
    fn range_ptr_rbracket_not_found<T>(&self) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::ExpectedKind {
                expected: "]",
                found: self.current_tkn().clone(),
            }
        )
    }
}
