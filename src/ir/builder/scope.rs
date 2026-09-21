//! スコープのノードを処理する


use super::*;

impl IR {
    /// スコープをスタート
    #[inline(always)]
    pub(in crate::ir::builder) fn begin_scope(
        &mut self
    ) {
        self.scope_states.push(self.var_tree.clone());
    }

    /// スコープが終了したときの処理
    /// 契約が終了しているかも検査
    pub(in crate::ir::builder) fn end_scope(
        &mut self,
        from_break: bool
    ) -> Result<(), err::ErrKind> {
        let states = if from_break {
            self.scope_states
                .iter()
                .rev()
                .collect::<Vec<_>>()
        } else {
            self.scope_states
                .last()
                .into_iter()
                .collect::<Vec<_>>()
        };

        for state in states {
            // 契約が終了しているかを確認
            for (name, var) in &self.var_tree.hash {
                if state.hash.contains_key(name)
                    || !var.size.is_constract_must()
                    || var.life != def_tree::VarLife::Constracting
                {
                    continue;
                }
                return crate::GenCompileErr!(
                    VariableConstractExpired, 
                    name
                );
            }
        }

        if from_break == false {
            let state = self.scope_states
                .pop()
                .expect("スコープが開始されていません");
            self.var_tree
                .hash
                .retain(
                    |name, _| {
                        state.hash.contains_key(name)
                    }
                );
        }
        Ok(())
    }

    /// `mod::func()`や`StructName.method()`のような、スコープを伴う
    /// 呼び出しを処理する
    ///
    /// ## `var_name`について
    /// この呼び出し(`Point.new()`など)の結果を格納する予定の
    /// 「元の変数名」があれば、それをここに渡す
    /// (例: `let p = Point.new();`や`p = Point.new();`の`p`)。
    /// - `Some`の場合: 構造体の初期化のために確保する一時的な
    ///   スタック領域を表す変数を、コンパイラ内部の合成名
    ///   (`__self_N`)ではなく、この`var_name`でそのまま
    ///   `var_tree`に登録する
    /// - `None`の場合: 対応する変数名が存在しない文脈
    ///   (関数の引数や構造体フィールドの初期化式として直接
    ///   使われた場合など)から呼ばれているので、内部的な
    ///   仮の名前を使う
    pub fn scope_node(
        &mut self,
        scope: &Vec<String>,
        target: Box<node::Expr>,
        var_name: Option<&String>,
        var_attr: &parse::VarMutAttr,
    ) -> Result<inst::Inst, err::ErrKind> {
        if let node::Expr::CallFunc(
            mut call_func_node
        ) = *target {
            if self.expr_counter != 1 {
                if let Some(struct_info) = self.struct_tree
                    .get(scope.last().unwrap())
                    .cloned() 
                {
                    let mut size = 0;
                    for field in &struct_info.fields {
                        // 確保するスタックを増やす
                        self.stack_counter(&field.ty);
                        size += self.size_of(&field.ty).to_bytes();
                    }

                    // 確保したスタックのノードをirに追加、targetの
                    // 引数の値に、自分自身のポインタを入れる、
                    self.ir_tree.push(inst::Inst::Stacks { size });
                    self.id_counter += 1;

                    self.ir_tree.push(inst::Inst::GetPtr {
                        size,
                        stk: self.stk_counter,
                    });
                    let self_idx = self.id_counter;
                    self.id_counter += 1;

                    let tmp_name = match var_name {
                        Some(name) => name.clone(),
                        None => format!("$self_area_{}", self_idx),
                    };
                    let _ = self.var_tree.push::<'l'>(
                        &tmp_name,
                        &self_idx,
                        &node::TyNode::Ty(struct_info.name.clone()),
                        &var_attr,
                    )?;

                    // メゾットの第一引数(`self`)として、今確保した
                    // スタックへのポインタを暗黙的に先頭へ渡す
                    call_func_node.args.insert(
                        0,
                        node::Expr::GetAddress(
                            Box::new(node::Expr::Var(tmp_name))
                        ),
                    );
                }
            }
            Ok(
                self.gen_call_fn_ir(
                    scope.last(), 
                    &call_func_node,
                    None
                )
            )
        } else {
            panic!();
        }
    }
}
