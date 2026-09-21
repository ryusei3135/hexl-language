use crate::ir::def_tree::FnDefInfo;

use super::*;

impl IR {
    /// 関数の戻り値の型や引数などの情報を登録し
    /// 処理のIRを生成する
    pub(super) fn ini_def_fn_info(
        &mut self, 
        info: &node::FuncDefine
    ) -> Result<(), err::ErrKind> {
        // 関数の情報を登録
        self.entry_fn_info(&info);
        self.var_tree = def_tree::VarTree::new();
        self.scope_states.clear();
        // `ir/param.rs`
        self.push_param_meta_data(&info.params)?;
        self.gen_inst(&info.body.clone());
        // 関数の処理内容をpush
        self.push_fn_ir_tree(&info);
        // 使うデータを初期化
        self.ir_tree = Vec::new();
        self.id_counter = 0;
        Ok(())
    }

    /// 関数の情報を関数ツリーに登録
    pub(super) fn push_fn_ir_tree(
        &mut self, 
        info: &node::FuncDefine
    ) {
        // 関数のデータをpush
        self.func_tree.add(
            // 関数の処理
            mem::take(&mut self.ir_tree),
            // 関数の引数や名前など
            &info,
            // 関数の戻り値
            &info.ret_ty,
            self.stk_counter,
        );
        self.stk_counter = 0;
    }

    /// 現在処理中の関数の情報を登録する
    /// **これは自分自身のファイルの中の関数**
    pub(super) fn entry_fn_info(
        &mut self, 
        info: &node::FuncDefine
    ) {
        self.check_must_var_used(&info);
        self.func_ret_ty = Some(info.ret_ty.clone());
        
        // メゾットの場合、`info.module`に自身が属する構造体の名前が
        // 入っているので、そのままモジュール名として登録する
        self.define_meta_data
            .push(
                def_tree::FnDefMetaData::new(
                    &info, 
                    info.module.as_ref()
                )
            );
        // 公開する関数を登録
        if info.public {
            self.public_func_tree
                .push(info.name.to_string());
        }
    }

    /// 外部の関数を定義するノードを
    /// 作成し、スタックする関数
    /// アセンブリ言語を出力する際にだけ使う
    pub(in crate::ir::builder) fn make_extern_func_inst(
        &mut self, 
        fn_tree: &Vec<def_tree::FnDefMetaData>
    ) {
        for func in fn_tree {
            self.extern_funcs
                .push(inst::Inst::ExternFunc(func.name.clone()));
        }
    }

    /// もし戻り値が契約ならフラグを立てる
    pub(super) fn gen_call_fn_ir(
        &mut self,
        module_name: Option<&String>,
        meta_data: &node::CallInfo,
        return_var_name: Option<&String>,
    ) -> inst::Inst {
        // 関数の定義を取得
        let defined_func_data: FnDefInfo = {
            if let Some(def_data) = self
                .func_tree
                .get_with_temp(
                    &meta_data.name, 
                    module_name,
                    &meta_data.temp_ty,
                )
            {
                def_data
            } else {
                let result = self.extern_func_tree
                    .iter()
                    .find(
                        |v| {
                            v.name.as_str() == meta_data.name.as_str() 
                            && v.module() == module_name
                        }
                    );
                if let Some(def_data) = result {
                    def_data.gen(self.stk_counter)
                } else {
                    panic!();
                }
            }
        };
        // もし戻り値が契約ならフラグを立てる
        self.is_constract_ret(&defined_func_data);

        let def_args = defined_func_data.args.clone();
        // 関数のノードを作成
        let mut func_meta_data = inst::CallFuncMetaData::new(
            meta_data.name.clone(),
            if self.expr_counter == 1 {
                IS_NOT_ASSIGN_EXPR
            } else {
                IS_ASSIGN_EXPR
            },
            // 確保するスタックのサイズを渡す
            if self.stk_counter != 0 {
                Some(self.stk_counter)
            } else {
                None
            },
        );

        if return_var_name.is_some() {
            if let Some(node::TyNode::Ty(struct_name)) 
                = defined_func_data
                    .ret_ty
                    .as_ref() 
            {
                if let Some(struct_info) = self
                    .struct_tree
                    .get(&struct_name)
                    .cloned() 
                {
                    let size = struct_info.fields
                        .iter()
                        .map(|field| self.size_of(&field.ty).to_bytes())
                        .sum();
                    self.stk_counter += size;
                    self.ir_tree.push(inst::Inst::Stacks { size });
                    self.id_counter += 1;

                    self.ir_tree.push(inst::Inst::GetPtr {
                        size,
                        stk: self.stk_counter,
                    });
                    let ptr_idx = self.id_counter;
                    self.id_counter += 1;

                    self.ir_tree.push(inst::Inst::GetAddress(ptr_idx));
                    let dst_idx = self.id_counter;
                    self.id_counter += 1;
                    func_meta_data.insert_param_parent_id(dst_idx);
                }
            }
        }

        for (index, _) in meta_data
            .args
            .iter()
            .enumerate() 
        {
            let expr_arg = meta_data
                .args
                .get(index)
                .unwrap()
                .clone();
            // 契約(`must`/`of`)のチェック
            // (`src/ir/ty_checker/constract.rs`)
            self.check_constract_arg(
                meta_data.name.as_str(), 
                &def_args[index], 
                &expr_arg
            ).unwrap();

            let ty = self.size_of(
                &def_args[index].ty
            ).clone();
            let idx = self.gen_expr_ir(expr_arg, &ty);
            func_meta_data.insert_param_parent_id(idx);
        }
        inst::Inst::CallFunc(func_meta_data)
    }

    #[inline(always)]
    fn is_constract_ret(
        &mut self,
        fn_def: &FnDefInfo
    ) {
        let flag = fn_def
            .ret_ty
            .as_ref()
            .map(|v| v.is_constract_must())
            .unwrap_or(false)
            || fn_def
                .ret_ty
                .as_ref()
                .map(|v| v.is_constract_of())
                .unwrap_or(false);
        if flag {
            self.constract_flag.constract_fn();
        }
    }
}
