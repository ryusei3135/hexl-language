//! スコープのノードを処理する

use super::*;

impl IR {
    /// スコープをスタート
    /// irに`StartScope`を挿入する
    #[inline(always)]
    pub(in crate::compiler::ir::builder) 
    fn begin_scope(&mut self) {
        self.scope_states.push(self.var_tree.clone());
        self.push_scope_inst(inst::Inst::StartScope);
    }

    /// ブロックの終わりで、スコープを終了する
    /// 契約の検査をしたあと、irに`EndScope`を挿入する
    pub(in crate::compiler::ir::builder) 
    fn end_block_scope(&mut self) {
        let result = self.end_scope(true);
        self.unwrap_or_report(result);
        self.push_scope_inst(inst::Inst::EndScope);
    }

    /// `StartScope`/`EndScope`をirに追加する
    /// (他のノードと同様に、`id_counter`も進める)
    #[inline(always)]
    fn push_scope_inst(&mut self, node: inst::Inst) {
        self.ir_tree.push(node);
        self.id_counter += 1;
    }

    /// スコープが終了したときの処理
    /// 契約が終了しているかも検査
    pub(in crate::compiler::ir::builder) 
    fn end_scope(
        &mut self,
        from_break: bool,
    ) -> Result<(), err::ErrKind> {
        let states = if from_break {
            self.scope_states.iter().rev().collect::<Vec<_>>()
        } else {
            self.scope_states.last().into_iter().collect::<Vec<_>>()
        };

        for state in states {
            // 契約が終了しているかを確認
            self.var_tree.all_var_is_end_constract(state)?;
        }

        if from_break == false {
            let state = self
                .scope_states
                .pop()
                .expect("スコープが開始されていません");
            self.var_tree.contains_key(&state);
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
    pub(super) 
    fn scope_node(
        &mut self,
        scope: &[String],
        target: Box<node::Expr>,
        var_name: Option<&String>,
        var_attr: &parse::VarMutAttr,
    ) -> Result<inst::Inst, err::ErrKind> {
        if let node::Expr::CallFunc(mut call_func_node) = *target {
            let struct_name = scope.last().unwrap();

            let needs_self_area = self
                .func_tree
                .get_with_temp(&call_func_node.name, Some(struct_name), &call_func_node.temp_ty)
                .or_else(|| {
                    self.extern_func_tree
                        .iter()
                        .find(|v| {
                            v.name.as_str() == call_func_node.name.as_str()
                                && v.module() == Some(struct_name)
                        })
                        .map(|def| def.gen_fn_def(self.stk_counter))
                })
                .is_some_and(|def| {
                    def.args.first().is_some_and(|arg| {
                        matches!(
                            &arg.ty,
                            node::TyNode::Pointer { ty_name, .. }
                                if matches!(&**ty_name, node::TyNode::Ty(name) if name == struct_name)
                        )
                    })
                });

            if needs_self_area {
                if let Some(struct_info) = self.struct_tree.get(struct_name).cloned() {
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
                    let self_area_ty = node::TyNode::Ty(struct_info.name.clone());
                    let _ = self.var_tree.push::<'l'>(
                        &tmp_name,
                        self_idx,
                        &self_area_ty,
                        &var_attr,
                        || self.constract_flag.put_var_def(&self_area_ty),
                    )?;

                    // メゾットの第一引数(`self`)として、今確保した
                    // スタックへのポインタを暗黙的に先頭へ渡す
                    call_func_node.args.insert(
                        0,
                        node::Expr::GetAddress(Box::new(node::Expr::Var(tmp_name))),
                    );
                }
            }
            Ok(self.gen_call_fn_ir(scope.last(), &call_func_node, None)?)
        } else {
            panic!();
        }
    }
}