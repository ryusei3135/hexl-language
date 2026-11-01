//! 関数のirを処理する

use crate::compiler::ir::def_tree::FnDefInfo;

use super::*;
use crate::compiler::ast::{self, CallInfo, FuncDefine, TyNode};

impl IR {
    /// 関数の戻り値の型や引数などの情報を登録し
    /// 処理のIRを生成する
    /// 一回しか呼ばれないのでinlineにしてもOk
    #[inline(always)]
    pub(super) fn init_def_fn_info<M: ResultMod>(
        &mut self,
        info: &FuncDefine,
    ) -> Result<(), err::ErrKind> {
        // 関数のスコープごとに使用回数を数え直す(`FnDefInfo.var_counter`に保存される)
        self.var_counter = counter_mnger::VarCounter::with_arg_count(info.params.len());
        // 関数の情報を登録
        self.entry_fn_info(&info)?;
        self.var_tree = def_tree::VarTree::new();
        self.scope_states.clear();
        // `ir/param.rs`
        self.push_param_meta_data(&info.params)?;
        // `:=`(`TyNode::Auto`)のうち、使われ方(`ret x`など)から型が決まるものを
        // 先に決めておく(`src/ir/builder/infer_auto.rs`)
        let mut body = info.body.clone();
        infer_auto::resolve_auto_by_usage(&mut body, &info.params, &info.ret_ty);
        self.gen_inst::<M>(&body)?;
        // 関数の処理内容をpush
        self.push_fn_ir_tree(&info);
        // 使うデータを初期化
        self.ir_tree = Vec::new();
        self.id_counter = 0;
        Ok(())
    }

    /// 関数の情報を関数ツリーに登録
    pub(super) fn push_fn_ir_tree(&mut self, info: &FuncDefine) {
        // 関数のデータをpush
        self.func_tree.add(
            // 関数の処理
            &self.ir_tree,
            // 関数の引数や名前など
            &info,
            // 関数の戻り値
            &info.ret_ty,
            self.stk_counter,
            &self.var_counter,
        );
        self.stk_counter = 0;
    }

    /// 現在処理中の関数の情報を登録する
    /// **これは自分自身のファイルの中の関数**
    pub(super) fn entry_fn_info(&mut self, info: &FuncDefine) -> Result<(), err::ErrKind> {
        // 前の関数の最後の文が立てた契約のフラグが、この関数の引数の登録
        // (`push_param_meta_data`)に持ち越されないようにする
        self.constract_flag.reset();
        self.check_must_var_used(&info)?;
        self.func_ret_ty = Some(info.ret_ty.clone());

        // メゾットの場合、`info.module`に自身が属する構造体の名前が
        // 入っているので、そのままモジュール名として登録する
        self.define_meta_data
            .push(def_tree::FnDefMetaData::new(&info, info.module.as_ref()));
        // 公開する関数を登録
        if info.public {
            self.public_func_tree.push(info.name.to_string());
        }
        Ok(())
    }

    /// 外部の関数を定義するノードを
    /// 作成し、スタックする関数
    /// アセンブリ言語を出力する際にだけ使う
    pub(super) fn make_extern_func_inst(&mut self, fn_tree: &[def_tree::FnDefMetaData]) {
        fn_tree.iter().for_each(|func| {
            self.extern_funcs
                .push(inst::Inst::ExternFunc(func.name.clone()));
        });
    }

    /// もし戻り値が契約ならフラグを立てる
    pub(super) fn gen_call_fn_ir<M: ResultMod>(
        &mut self,
        module_name: Option<&str>,
        meta_data: &CallInfo,
        return_var_name: Option<&String>,
    ) -> Result<inst::Inst, err::ErrKind> {
        // 関数の定義を取得
        let defined_func_data: FnDefInfo = {
            if let Some(def_data) =
                self.func_tree
                    .get_with_temp(&meta_data.name, module_name, &meta_data.temp_ty)
            {
                def_data
            } else {
                let result = self.extern_func_tree.iter().find(|v| {
                    v.name.as_str() == meta_data.name.as_str() && v.module() == module_name
                });
                if let Some(def_data) = result {
                    def_data.gen_fn_def(self.stk_counter)
                } else {
                    // 呼び出しのある文の位置(ファイル名を含む)を付ける
                    return Err(undef::UndefErrs::undef_fn_at(
                        &meta_data.name,
                        Rc::clone(&self.current_span),
                    ));
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
        func_meta_data.temp_ty = meta_data.temp_ty.clone();

        if return_var_name.is_some() {
            if let Some(TyNode::Ty(struct_name)) = defined_func_data.ret_ty.as_ref() {
                if let Some(struct_info) = self.struct_tree.get(&struct_name).cloned() {
                    let size = struct_info
                        .fields
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

        for (index, _) in meta_data.args.iter().enumerate() {
            let expr_arg = meta_data.args.get(index).unwrap().clone();
            // 契約(`must`/`of`)のチェック
            // (`src/ir/ty_checker/constract.rs`)
            self.check_constract_arg(meta_data.name.as_str(), &def_args[index], &expr_arg)?;

            let ty = self.size_of(&def_args[index].ty).clone();
            // `data"len"`: 文字列リテラルの長さは、静的データ`M0Len`にして参照する
            let idx = match self.gen_str_len_arg_ir(&expr_arg, &ty) {
                Some(idx) => idx,
                None => self.gen_expr_ir::<M>(&expr_arg, Some(&ty))?,
            };
            func_meta_data.insert_param_parent_id(idx);
        }
        Ok(inst::Inst::CallFunc(func_meta_data))
    }

    /// 関数の引数`data"len"`(`GetContractMemLen`)が、文字列リテラルの長さなら
    /// 静的データ`M{n}Len`を参照する`Inst::StrLen`を作り、そのidを返す
    ///
    /// ```text
    /// M0     "hello world"
    /// M0Len  11
    /// ```
    ///
    /// - 文字列リテラルの長さを直接指定した場合(`"hello world"`の長さ): `Str`も作る
    /// - 変数`data`の場合: 変数を定義したときの`Str`(`M{n}`)を使い回す。
    ///   変更できる変数(`mut`)は、別の値に変わっている可能性があるので対象外
    ///
    /// 長さが静的に分からない場合は`None`(従来どおり、定数畳み込みなどで処理する)
    fn gen_str_len_arg_ir(&mut self, arg: &ast::Expr, size: &types::Size) -> Option<usize> {
        let ast::ExprNode::GetContractMemLen(target) = &arg.node else {
            return None;
        };
        let (str_id, len) = match &target.node {
            ast::ExprNode::Str(value) => {
                let str_id = self.id_counter;
                self.ir_tree.push(inst::Inst::Str {
                    dst: str_id,
                    value: value.to_string(),
                });
                self.id_counter += 1;
                (str_id, value.len())
            }
            ast::ExprNode::Var(name) => {
                if !matches!(self.var_tree.is_mut(name), parse::VarMutAttr::Invar) {
                    return None;
                }
                let Ok(meta) = self.var_tree.get_var(name) else {
                    return None;
                };
                // 変数の定義(`Mov`)から、値の`Str`を辿る
                let def_tree::VarType::Local(def_idx) = meta.attribute else {
                    return None;
                };
                let Some(inst::Inst::Mov { src, .. }) = self.ir_tree.get(def_idx) else {
                    return None;
                };
                let Some(inst::Inst::Str { value, .. }) = self.ir_tree.get(*src) else {
                    return None;
                };
                let found = (*src, value.len());
                self.count_var_use(name);
                found
            }
            _ => return None,
        };
        let dst = self.id_counter;
        // 長さの情報を作成
        self.ir_tree.push(inst::Inst::StrLen {
            dst,
            str_id,
            len: len - 1,
            size: size.clone(),
        });
        self.id_counter += 1;
        Some(dst)
    }

    #[inline(always)]
    fn is_constract_ret(&mut self, fn_def: &FnDefInfo) {
        let flag = fn_def
            .ret_ty
            .as_ref()
            .map(|v| v.is_contract_must())
            .unwrap_or(false)
            || fn_def
                .ret_ty
                .as_ref()
                .map(|v| v.is_contract_of())
                .unwrap_or(false);
        if flag {
            self.constract_flag.constract_fn(fn_def.ret_ty.as_ref());
        }
    }
}

#[cfg(test)]
mod str_len_arg_tests {
    use super::*;

    fn build_func_body(src: &str) -> Vec<inst::Inst> {
        let nodes = parse_test_source(src);
        let mut ir = new_test_ir();
        ir.builder::<crate::compiler::ir::result_mod::PanicErr>(&nodes)
            .unwrap();
        ir.test_only_get_func_body("_start").to_vec()
    }

    /// `data"len"`を関数の引数にすると、`Str`(`M0`)と対になる`StrLen`(`M0Len`)を参照する
    #[test]
    fn str_len_arg_refers_to_static_len() {
        let body = build_func_body(
            "write(p: [b1 11], n: uint64): int { ret 0 } \
             _start(): int { data: [b1 11] = \"hello world\" write(data, data\"len\") ret 0 }",
        );
        let str_id = body.iter().position(|i| matches!(i, inst::Inst::Str { .. }));
        let str_len = body.iter().find_map(|i| match i {
            inst::Inst::StrLen { str_id, len, .. } => Some((*str_id, *len)),
            _ => None,
        });
        assert_eq!(str_len, str_id.map(|id| (id, 11)), "{:?}", body);
    }
}
