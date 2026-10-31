//! 確定していない式の型を確定させる
//! おもに数字リテラルを処理する

use super::*;


impl IR {
    /// 型が決まっていない即値(`10`や`1.5`、即値だけの式`1 + 2`)か
    pub(in super) fn is_untyped_literal(expr: &Expr) -> bool {
        match &expr.node {
            ExprNode::Number(_) | ExprNode::Float(_) => true,
            ExprNode::Add(n)
            | ExprNode::Sub(n)
            | ExprNode::Mul(n)
            | ExprNode::Div(n)
            | ExprNode::Surplus(n)
            | ExprNode::LShift(n)
            | ExprNode::RShift(n)
            | ExprNode::AndCalcul(n)
            | ExprNode::XorCalcul(n)
            | ExprNode::OrCalcul(n) => {
                Self::is_untyped_literal(&n.0) && Self::is_untyped_literal(&n.1)
            }
            // `-1`や`~0`のような、即値だけの単項演算
            ExprNode::Sign((_, target))
            | ExprNode::Inversion(target)
            | ExprNode::CmpNot(target) => Self::is_untyped_literal(&target),
            _ => false,
        }
    }

    /// 即値の型を決める元になる、式の型(定義元の型)を調べる
    ///
    /// - 変数は、定義した型
    /// - 二項演算は、型が分かるほうのオペランドの型
    /// - 数値型(整数・浮動小数点)だけを返す。ポインタ・構造体などは
    ///   即値の型にできないので`None`
    ///
    /// 副作用が無い(変数の使用回数も数えない)ので、`&self`で調べる
    pub(in super::super) fn infer_expr_ty(&self, expr: &Expr) -> Option<types::Size> {
        let ty = match &expr.node {
            ExprNode::Var(name) => {
                let ty_node = self.var_tree.get_ty_node(name)?;
                self.try_size_or_emit_struct(&ty_node).ok()?
            }
            ExprNode::Inc(target) | ExprNode::Dec(target) => {
                return self.infer_expr_ty(target);
            }
            // 単項演算の結果は、オペランドと同じ型
            // (`!a`は比較の結果なので、型を決める元にしない)
            ExprNode::Sign((_, target))
            | ExprNode::Inversion(target)
            | ExprNode::LeftInc(target)
            | ExprNode::LeftDec(target) => {
                return self.infer_expr_ty(target);
            }
            // キャストの結果は、キャスト先の型(組み込み型のみ)
            ExprNode::Cast((ty, _)) => match ty {
                TyNode::Ty(_) => types::Size::try_from(ty).ok()?,
                _ => return None,
            },
            ExprNode::Add(n)
            | ExprNode::Sub(n)
            | ExprNode::Mul(n)
            | ExprNode::Div(n)
            | ExprNode::Surplus(n)
            | ExprNode::LShift(n)
            | ExprNode::RShift(n)
            | ExprNode::AndCalcul(n)
            | ExprNode::XorCalcul(n)
            | ExprNode::OrCalcul(n) => self
                .infer_expr_ty(&n.0)
                .or_else(|| self.infer_expr_ty(&n.1))?,
            _ => return None,
        };
        use types::Size;
        matches!(
            ty,
            Size::DB
                | Size::DW
                | Size::DD
                | Size::DQ
                | Size::UDB
                | Size::UDW
                | Size::UDD
                | Size::UDQ
                | Size::F32
                | Size::F64
        )
        .then_some(ty)
    }

    /// 二項演算の左右のオペランドに渡す、期待する型を決める
    ///
    /// 片方が型の決まっていない即値で、もう片方の型が分かるなら、
    /// 即値の側はもう片方の型(定義元の型)にする
    /// (`a: uint64`に対する`a < 10`や`10 < a`の`10`は`uint64`になる)。
    /// 整数の即値は整数型へ、小数の即値は浮動小数点型へだけ変換する。
    /// それ以外は、渡された`expect_byte`をそのまま使う
    pub(in crate::compiler::ir::builder) fn operand_expect_tys(
        &self,
        ls: &Expr,
        rs: &Expr,
        expect_byte: &types::Size,
    ) -> (types::Size, types::Size) {
        let fit = |lit: &Expr, other: &Expr| -> Option<types::Size> {
            if Self::is_untyped_literal(lit) == false || Self::is_untyped_literal(other) {
                return None;
            }
            let ty = self.infer_expr_ty(other)?;
            let lit_is_float = Self::literal_has_float(lit);
            (ty.is_float() == lit_is_float).then_some(ty)
        };
        let l = fit(ls, rs).unwrap_or_else(|| expect_byte.clone());
        let r = fit(rs, ls).unwrap_or_else(|| expect_byte.clone());
        (l, r)
    }

    /// 即値の式の中に小数(`Expr::Float`)があるか
    pub(in super) fn literal_has_float(expr: &Expr) -> bool {
        match &expr.node {
            ExprNode::Float(_) => true,
            ExprNode::Add(n)
            | ExprNode::Sub(n)
            | ExprNode::Mul(n)
            | ExprNode::Div(n)
            | ExprNode::Surplus(n)
            | ExprNode::LShift(n)
            | ExprNode::RShift(n)
            | ExprNode::AndCalcul(n)
            | ExprNode::XorCalcul(n)
            | ExprNode::OrCalcul(n) => {
                Self::literal_has_float(&n.0) || Self::literal_has_float(&n.1)
            }
            ExprNode::Sign((_, target)) => Self::literal_has_float(target),
            _ => false,
        }
    }

    /// `gen_expr_ir`に渡された期待する型を確定させる
    ///
    /// - `Assign`ノードなら、代入先の型で確定させる
    ///   - 構造体のメンバー(`var.member` / `[ptr].member` /
    ///     `var.[member idx]`)への代入なら、そのメンバーの型
    ///   - それ以外は、代入先の変数の型
    /// - それ以外は、渡された型をそのまま使う
    /// - どちらも無い(`None`で`Assign`でもない)なら、二項演算は
    ///   オペランドの変数の型(定義元の型)、それ以外は`DD`にする
    pub(in crate::compiler::ir::builder) fn resolve_expect_byte<M: ResultMod>(
        &mut self,
        expr: &Expr,
        expect_byte: Option<&types::Size>,
    ) -> Result<types::Size, err::ErrKind> {
        if let ExprNode::Assign(assign_node) = &expr.node {
            if let Some(size) = self.member_ty_of_assign_dst::<M>(&assign_node.dst, expect_byte)? {
                return Ok(size);
            }
            if let Some(ty_node) = self.var_tree.get_ty_node(&assign_node.name) {
                let result = self.try_size_or_emit_struct(&ty_node);
                return Ok(self.unwrap_or_report(result));
            }
        }
        Ok(expect_byte
            .cloned()
            .or_else(|| {
                // 型が決まっていない二項演算(`if a < 10`など)は、
                // オペランドの型で決める
                match expr.node {
                    ExprNode::Number(_) | ExprNode::Float(_) => None,
                    // 比較(`n % 15 == 0`/`i < 10`)は、結果ではなくオペランドの型で決める
                    ExprNode::LessThen(ref n)
                    | ExprNode::GreaterThen(ref n)
                    | ExprNode::LessThenEq(ref n)
                    | ExprNode::GreaterThenEq(ref n)
                    | ExprNode::Equal(ref n)
                    | ExprNode::NotEq(ref n) => self
                        .infer_expr_ty(&n.0)
                        .or_else(|| self.infer_expr_ty(&n.1)),
                    _ => self.infer_expr_ty(expr),
                }
            })
            .unwrap_or(types::Size::DD))
    }
}