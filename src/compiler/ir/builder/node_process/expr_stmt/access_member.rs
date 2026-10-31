use super::*;


impl IR {
    /// `[ptr->member index else else_idx]`(`PtrMember`の`target`が`RefArray`)を、
    /// ポインタを一時変数に入れた通常の配列アクセスに展開する
    ///
    /// ```text
    /// $ptr_arr_N: <メンバーの型> = ptr->member      // ここで`DefVar`のIRを出力する
    /// [$ptr_arr_N index else else_idx]               // 戻り値の式
    /// ```
    ///
    /// ポインタ変数への配列アクセス(`[p index]`)と同じ処理に乗せるための展開で、
    /// `PtrMember`の`RefArray`をそのままIRにすると、添字が捨てられてメンバー自体への
    /// アクセスになってしまう。
    ///
    /// 戻り値は、(一時変数の名前, 配列アクセスの式)
    pub(in super) fn ptr_member_arr_to_tmp<M: ResultMod>(
        &mut self,
        dst: &Expr,
    ) -> Result<(String, Expr), err::ErrKind> {
        let ExprNode::PtrMember {
            name: ptr, target, ..
        } = &dst.node
        else {
            return Err(err::ErrKind::UnexpectedToken);
        };
        let ExprNode::RefArray {
            name: member,
            index,
            else_idx,
            ..
        } = &target.node
        else {
            return Err(err::ErrKind::UnexpectedToken);
        };

        // メンバーの型(ポインタ/配列)。`get_ty_name`は、ポインタ変数には指す先の構造体名を返す
        let struct_name = self.var_tree.get_ty_name(ptr)?;
        let field_ty = self
            .struct_tree
            .get(&struct_name)
            .ok_or_else(|| crate::GenUndefErrResult!(UndefStruct, struct_name.to_string(), None))?
            .fields
            .iter()
            .find(|field| field.name == *member)
            .ok_or_else(|| crate::GenUndefErrResult!(UndefStructMember, member.to_string(), None))?
            .ty
            .clone();
        let field_size = M::handle(self.try_size_or_emit_struct(&field_ty))?;

        // 一時変数は、メンバーの値(ポインタ)のコピーにすぎないので、メンバーの契約
        // (`T* must=free`など)は引き継がない。契約の終了は、構造体を初期化した
        // スコープの`del`で行うもので、一時変数に契約を持たせると、
        // 要素の代入が「`must`変数への再代入」になったり、一時変数の契約終了を
        // 求められたりする
        let tmp_ty = match &field_ty {
            TyNode::ConstractMust(inner) | TyNode::ConstractOf(inner) => inner.unwrap_ty(),
            other => other.clone(),
        };
        // 要素の書き込み可否は、ポインタ変数(`self`など)の可変性に従う
        let tmp_attr = self.var_tree.is_mut(ptr);

        let span = Rc::clone(&dst.span);
        let tmp = format!("$ptr_arr_{}", self.id_counter);
        // `ptr->member`(メンバーそのものの値 = 配列の先頭を指すポインタ)
        let member_value = Expr::new(
            ExprNode::PtrMember {
                name: ptr.clone(),
                index: None,
                else_idx: None,
                target: Box::new(Expr::new(ExprNode::Var(member.clone()), Rc::clone(&span))),
            },
            Rc::clone(&span),
        );
        let def_var = Expr::new(
            ExprNode::DefVar(DefineVar {
                name: tmp.clone(),
                value: Box::new(member_value),
                ty: tmp_ty,
                var_attr: tmp_attr,
            }),
            Rc::clone(&span),
        );
        self.gen_expr_ir::<M>(&def_var, Some(&field_size))?;

        let tmp_var = Expr::new(ExprNode::Var(tmp.clone()), Rc::clone(&span));
        let arr = Expr::new(
            ExprNode::RefArray {
                name: tmp.clone(),
                dst: Box::new(tmp_var),
                index: index.clone(),
                else_idx: else_idx.clone(),
            },
            span,
        );
        Ok((tmp, arr))
    }


    /// `value`が、ポインタ`ptr_name`の指す構造体のメンバー`member`を
    /// 引数に含む関数呼び出し(`f(self->val, ..)` / `mod::f<T>(self->val, ..)`)か
    pub(in super) fn value_hands_over_member(value: &Expr, ptr_name: &str, member: &str) -> bool {
        fn is_member(arg: &Expr, ptr_name: &str, member: &str) -> bool {
            match &arg.node {
                ExprNode::ConnectAddr(target) => is_member(target, ptr_name, member),
                ExprNode::PtrMember { name, target, .. } => {
                    name.as_str() == ptr_name
                        && matches!(&target.node, ExprNode::Var(m) if m.as_str() == member)
                }
                _ => false,
            }
        }

        let call = match &value.node {
            ExprNode::CallFunc(call) => call,
            ExprNode::Scope { target, .. } => match &target.node {
                ExprNode::CallFunc(call) => call,
                _ => return false,
            },
            _ => return false,
        };
        call.args.iter().any(|arg| is_member(arg, ptr_name, member))
    }
}