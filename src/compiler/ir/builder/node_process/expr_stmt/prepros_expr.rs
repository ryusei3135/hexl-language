//! 前置の単項演算子(`+` `-` `!` `~` `++` `--`)などを処理する関数を提供

use super::*;


impl IR {
    /// 前置の単項演算子(`+` `-` `!` `~` `++` `--`)を、既存のノードに展開する
    ///
    /// 単項演算専用の命令は作らず、バックエンドが対応済みの命令だけで表す
    /// - `+a`  -> `a`
    /// - `-a`  -> `0 - a`
    /// - `~a`  -> `a ^ (全ビットが1の値)`
    /// - `!a`  -> `a == 0`
    /// - `++a` -> `a = a + 1`(式の値は、加算後の値)
    /// - `--a` -> `a = a - 1`(式の値は、減算後の値)
    /// - `self->len++` / `a.len--`など、メンバーの後置`++`/`--`も、同じく代入に展開する
    ///   (`a`が変数の後置は`build_inc_dec_expr`)
    ///
    /// 単項演算のノードでなければ`Ok(None)`
    pub(in crate::compiler::ir::builder) fn desugar_unary<M: ResultMod>(
        &mut self,
        expr: &Expr,
        expect_byte: Option<&types::Size>,
    ) -> Result<Option<Expr>, err::ErrKind> {
        // オペランドの型。分からなければ呼び出し側の期待する型
        let operand_ty = |this: &Self, target: &Expr| -> Option<types::Size> {
            this.infer_expr_ty(target).or_else(|| expect_byte.cloned())
        };

        let expanded = match &expr.node {
            ExprNode::Sign((true, target)) => (**target).clone(),
            ExprNode::Sign((false, target)) => {
                let is_float = operand_ty(self, target).is_some_and(|ty| ty.is_float())
                    || Self::literal_has_float(target);
                let zero = if is_float {
                    ExprNode::Float("0.0".to_string()).with_span(Rc::clone(&expr.span))
                } else {
                    ExprNode::Number("0".to_string()).with_span(Rc::clone(&expr.span))
                };
                ExprNode::Sub(Expr::wrap(zero, (**target).clone())).with_span(Rc::clone(&expr.span))
            }
            ExprNode::Inversion(target) => {
                use types::Size;
                // 全ビットが1の値(符号付きは-1、符号なしは型の最大値)
                let mask = match operand_ty(self, target).unwrap_or(Size::DD) {
                    Size::UDB => u8::MAX.to_string(),
                    Size::UDW => u16::MAX.to_string(),
                    Size::UDD => u32::MAX.to_string(),
                    Size::UDQ => u64::MAX.to_string(),
                    Size::F32 | Size::F64 => return Err(err::ErrKind::UnexpectedToken),
                    _ => "-1".to_string(),
                };
                ExprNode::XorCalcul(Expr::wrap(
                    (**target).clone(),
                    ExprNode::Number(mask).with_span(Rc::clone(&expr.span)),
                ))
                .with_span(Rc::clone(&expr.span))
            }
            ExprNode::CmpNot(target) => ExprNode::Equal(Expr::wrap(
                (**target).clone(),
                ExprNode::Number("0".to_string()).with_span(Rc::clone(&expr.span)),
            ))
            .with_span(Rc::clone(&expr.span)),
            // `++a` / `--a`
            ExprNode::LeftInc(target) | ExprNode::LeftDec(target) => {
                self.desugar_inc_dec::<M>(expr, target, matches!(&expr.node, ExprNode::LeftInc(_)))?
            }
            // 構造体のメンバーの`a->len++` / `a.len--`(変数の`a++`は`build_inc_dec_expr`が処理する)
            ExprNode::Inc(target) | ExprNode::Dec(target)
                if matches!(
                    &target.node,
                    ExprNode::PtrMember { .. } | ExprNode::Member { .. }
                ) =>
            {
                self.desugar_inc_dec::<M>(expr, target, matches!(&expr.node, ExprNode::Inc(_)))?
            }
            // `[ptr->member index] = value`: 一時変数に入れたポインタへの配列の代入に展開する
            ExprNode::Assign(assign)
                if matches!(
                    &assign.dst.node,
                    ExprNode::PtrMember { target, .. }
                        if matches!(target.node, ExprNode::RefArray { .. })
                ) =>
            {
                let (tmp, arr) = self.ptr_member_arr_to_tmp::<M>(&assign.dst)?;
                AssignVar::new(&tmp, arr, (*assign.value).clone(), Rc::clone(&expr.span))
            }
            // `[ptr->member index]`(値の読み取り)
            ExprNode::PtrMember { target, .. }
                if matches!(target.node, ExprNode::RefArray { .. }) =>
            {
                self.ptr_member_arr_to_tmp::<M>(expr)?.1
            }
            _ => return Ok(None),
        };
        Ok(Some(expanded))
    }
}