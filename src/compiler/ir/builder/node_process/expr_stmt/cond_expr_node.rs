//! 条件で結果がからる式を処理する関数を提供する

use super::*;

impl IR {
    /// 三項演算子の条件式として使える式か
    ///
    /// - 比較(`<` `>` `<=` `>=` `==` `!=`)・`&&`・`||`・`!`
    /// - 比較の両辺は、メモリにアクセスしない式(`is_mem_free`)だけ
    ///   (配列・ポインタの中身・メンバー・関数呼び出し・代入・`++`/`--`は不可)
    fn is_ternary_cond(expr: &Expr) -> bool {
        match &expr.node {
            ExprNode::CmpAnd(n) | ExprNode::CmpOr(n) => {
                Self::is_ternary_cond(&n.0) && Self::is_ternary_cond(&n.1)
            }
            ExprNode::CmpNot(target) => Self::is_ternary_cond(target),
            _ => Self::is_mem_free(expr),
        }
    }

    /// 生成した命令を`ir_tree`へ積み、その命令の番号を返す
    fn push_ternary_inst(&mut self, inst: inst::Inst) -> usize {
        self.ir_tree.push(inst);
        self.id_counter += 1;
        self.id_counter - 1
    }

    /// `ExprKind::Ternary*`(比較の結果を`Ternary`が使うための命令)を作る
    fn push_ternary_cmp(&mut self, kind: inst::ExprKind, ls: usize, rs: usize) -> usize {
        let inst = inst::ExprInst {
            dst: self.id_counter,
            ls,
            rs,
            kind,
        }
        .new();
        self.push_ternary_inst(inst)
    }

    /// 三項演算子の条件式から、`ExprKind::Ternary*`の命令を生成する
    /// 戻り値は、生成した`Ternary*`命令の番号
    ///
    /// - 比較が1つだけ(`a < b` `a != b` など)
    ///   → その比較に対応する`TernaryLessThen` `TernaryGreaterThen`
    ///     `TernaryLessThenEq` `TernaryGreaterThenEq` `TernaryEqual` `TernaryNotEq`を1命令だけ生成
    /// - `&&` / `||` がある(`a < b && c != d` など)
    ///   → 各比較を通常の比較(0/1)で作り、`&&`は`AndCalcul`、`||`は`OrCalcul`で畳み込んでから
    ///     `TernaryNotEq(結果, 0)`を生成する
    ///   → `!(..)`は`TernaryEqual(結果, 0)`を生成する
    /// - 比較ではない式(`x` `x + 1`など)は`TernaryNotEq(式, 0)`
    ///
    /// ポインタのアクセス・関数呼び出しなどを含む条件式は受け付けない(panic)。
    /// 呼び出し側は先に`is_ternary_cond`で確認すること。
    pub(in crate::compiler::ir) fn gen_ternary_cond_ir<M: ResultMod>(
        &mut self,
        cond: &Expr,
    ) -> Result<usize, err::ErrKind> {
        if Self::is_ternary_cond(cond) == false {
            panic!(
                "三項演算子の条件式には、ポインタのアクセスや関数呼び出しなどは使えません: {:?}",
                cond
            );
        }

        // 条件式全体の型(`gen_expr_ir`と同じ決め方)
        let size = self.resolve_expect_byte::<M>(cond, None)?;

        match &cond.node {
            // 比較が1つだけ
            ExprNode::LessThen(n) => {
                self.gen_ternary_cmp_ir::<M>(n, &size, inst::ExprKind::TernaryLessThen)
            }
            ExprNode::GreaterThen(n) => {
                self.gen_ternary_cmp_ir::<M>(n, &size, inst::ExprKind::TernaryGreaterThen)
            }
            ExprNode::LessThenEq(n) => {
                self.gen_ternary_cmp_ir::<M>(n, &size, inst::ExprKind::TernaryLessThenEq)
            }
            ExprNode::GreaterThenEq(n) => {
                self.gen_ternary_cmp_ir::<M>(n, &size, inst::ExprKind::TernaryGreaterThenEq)
            }
            ExprNode::Equal(n) => {
                self.gen_ternary_cmp_ir::<M>(n, &size, inst::ExprKind::TernaryEqual)
            }
            ExprNode::NotEq(n) => {
                self.gen_ternary_cmp_ir::<M>(n, &size, inst::ExprKind::TernaryNotEq)
            }

            // `&&` / `||`: 結果(0/1)が0でないか
            ExprNode::CmpAnd(..) | ExprNode::CmpOr(..) => {
                let value = self.gen_setcc_ir::<M>(cond)?;
                let zero = self.push_ternary_inst(inst::Inst::gen_num(
                    &"0".to_string(),
                    &size,
                    self.id_counter,
                ));
                Ok(self.push_ternary_cmp(inst::ExprKind::TernaryNotEq, value, zero))
            }

            // `!(..)`: 結果(0/1)が0か
            ExprNode::CmpNot(target) => {
                let value = self.gen_setcc_ir::<M>(&*target)?;
                let zero = self.push_ternary_inst(inst::Inst::gen_num(
                    &"0".to_string(),
                    &size,
                    self.id_counter,
                ));
                Ok(self.push_ternary_cmp(inst::ExprKind::TernaryEqual, value, zero))
            }

            // 比較ではない式: 0でないか
            _ => {
                let value = self.gen_expr_ir::<M>(cond, Some(&size))?;
                let zero = self.push_ternary_inst(inst::Inst::gen_num(
                    &"0".to_string(),
                    &size,
                    self.id_counter,
                ));
                Ok(self.push_ternary_cmp(inst::ExprKind::TernaryNotEq, value, zero))
            }
        }
    }

    /// 比較1つ分の`Ternary*`命令
    fn gen_ternary_cmp_ir<M: ResultMod>(
        &mut self,
        node: &(Box<Expr>, Box<Expr>),
        size: &types::Size,
        kind: inst::ExprKind,
    ) -> Result<usize, err::ErrKind> {
        // 即値の型は、もう片方のオペランドに合わせる(`left_right_pair`)
        let (ls, rs) = self.left_right_pair::<M>(node, size)?;
        Ok(self.push_ternary_cmp(kind, ls, rs))
    }

    /// `Set*`(setcc)命令で、`expr`を0か1の値にする必要があるか
    ///
    /// 次の場合に`true`になる
    /// - `bool`型への代入・引数・戻り値など、期待する型が`bool`の比較(`b: bool = a < c`)
    /// - 条件式が複雑な場合: `&&` / `||`、または`!`の中が比較・`&&` / `||` / `!`
    /// - 三項演算子の複雑な条件式(`gen_ternary_cond_ir`)
    ///
    /// それ以外の単純な比較(`if a < b`や三項演算子の`a < b ? x : y`)は、
    /// 今までどおり`LessThen`や`Ternary*`を使う
    pub(in crate::compiler::ir) fn needs_setcc(
        expr: &Expr,
        expect_byte: Option<&types::Size>,
    ) -> bool {
        let is_bool = matches!(expect_byte, Some(types::Size::Bool));
        match &expr.node {
            ExprNode::CmpAnd(..) | ExprNode::CmpOr(..) => true,
            ExprNode::CmpNot(target) => {
                is_bool || Self::is_cmp_expr(target) || Self::is_logic_expr(target)
            }
            _ => is_bool && Self::is_cmp_expr(expr),
        }
    }

    /// 比較(`<` `>` `<=` `>=` `==` `!=`)か
    fn is_cmp_expr(expr: &Expr) -> bool {
        match &expr.node {
            ExprNode::LessThen(..)
            | ExprNode::GreaterThen(..)
            | ExprNode::LessThenEq(..)
            | ExprNode::GreaterThenEq(..)
            | ExprNode::Equal(..)
            | ExprNode::NotEq(..) => true,
            _ => false,
        }
    }

    /// `&&` / `||` / `!`か
    fn is_logic_expr(expr: &Expr) -> bool {
        match &expr.node {
            ExprNode::CmpAnd(..) | ExprNode::CmpOr(..) | ExprNode::CmpNot(..) => true,
            _ => false,
        }
    }

    /// 比較のオペランドの型(`Set*`の両辺に渡す、期待する型)
    ///
    /// 変数などオペランドの型が分かればその型、分からなければ`DD`。
    /// `bool`への代入でも、オペランドの型は`bool`にならない(`check_var_ty`で弾かれる)
    fn set_cmp_operand_ty(&self, node: &(Box<Expr>, Box<Expr>)) -> types::Size {
        self.infer_expr_ty(&node.0)
            .or_else(|| self.infer_expr_ty(&node.1))
            .unwrap_or(types::Size::DD)
    }

    /// 比較1つ分の`Set*`命令(結果は0か1)
    fn gen_set_cmp_ir<M: ResultMod>(
        &mut self,
        node: &(Box<Expr>, Box<Expr>),
        kind: inst::ExprKind,
    ) -> Result<usize, err::ErrKind> {
        let size = self.set_cmp_operand_ty(&node);
        let (ls, rs) = self.left_right_pair::<M>(node, &size)?;
        Ok(self.push_ternary_cmp(kind, ls, rs))
    }

    /// 比較・`&&` / `||` / `!`を、0か1の値を作る`Set*`命令へ展開して、その値の番号を返す
    ///
    /// - `a < b`  -> `SetLessThen(a, b)`(`>` `<=` `>=` `==` `!=`も同様に`Set*`)
    /// - `x && y` -> `AndCalcul(<x>, <y>)`
    /// - `x || y` -> `OrCalcul(<x>, <y>)`
    /// - `!x`     -> `SetEqual(<x>, 0)`
    /// - 比較ではない式`x` -> `SetNotEq(x, 0)`(`bool`型の変数は0/1なのでそのまま使う)
    ///
    /// `&&` / `||`は、両辺を必ず評価する(短絡評価はしない)。
    /// 右辺に副作用やメモリアクセスがあるときは、分岐で展開する必要がある
    pub(in crate::compiler::ir) fn gen_setcc_ir<M: ResultMod>(
        &mut self,
        expr: &Expr,
    ) -> Result<usize, err::ErrKind> {
        use inst::ExprKind as K;
        self.expr_counter += 1;
        match &expr.node {
            ExprNode::LessThen(n) => self.gen_set_cmp_ir::<M>(n, K::SetLessThen),
            ExprNode::GreaterThen(n) => self.gen_set_cmp_ir::<M>(n, K::SetGreaterThen),
            ExprNode::LessThenEq(n) => self.gen_set_cmp_ir::<M>(n, K::SetLessThenEq),
            ExprNode::GreaterThenEq(n) => self.gen_set_cmp_ir::<M>(n, K::SetGreaterThenEq),
            ExprNode::Equal(n) => self.gen_set_cmp_ir::<M>(n, K::SetEqual),
            ExprNode::NotEq(n) => self.gen_set_cmp_ir::<M>(n, K::SetNotEq),
            ExprNode::CmpAnd((l, r)) => {
                let ls = self.gen_setcc_ir::<M>(&*l)?;
                let rs = self.gen_setcc_ir::<M>(&*r)?;
                Ok(self.push_ternary_cmp(K::AndCalcul, ls, rs))
            }
            ExprNode::CmpOr((l, r)) => {
                let ls = self.gen_setcc_ir::<M>(&*l)?;
                let rs = self.gen_setcc_ir::<M>(&*r)?;
                Ok(self.push_ternary_cmp(K::OrCalcul, ls, rs))
            }
            ExprNode::CmpNot(target) => {
                let value = self.gen_setcc_ir::<M>(&*target)?;
                let bool_ty = types::Size::Bool;
                let zero =
                    self.push_ternary_inst(inst::Inst::gen_num("0", &bool_ty, self.id_counter));
                Ok(self.push_ternary_cmp(K::SetEqual, value, zero))
            }
            // 比較ではない式
            _ => {
                // `bool`型の変数は、すでに0か1
                if let ExprNode::Var(name) = &expr.node {
                    let is_bool = self
                        .var_tree
                        .get_ty_node(name)
                        .and_then(|ty| self.try_size_or_emit_struct(&ty).ok())
                        .is_some_and(|ty| ty == types::Size::Bool);
                    if is_bool {
                        return self.gen_expr_ir::<M>(&expr, Some(&types::Size::Bool));
                    }
                }
                let size = self.infer_expr_ty(&expr).unwrap_or(types::Size::DD);
                let value = self.gen_expr_ir::<M>(&expr, Some(&size))?;
                let zero = self.push_ternary_inst(inst::Inst::gen_num("0", &size, self.id_counter));
                Ok(self.push_ternary_cmp(K::SetNotEq, value, zero))
            }
        }
    }
}