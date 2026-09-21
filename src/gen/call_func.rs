mod assign_var;
mod mem_ir;

use super::*;
use crate::gen::emit_fn_name::*;
use crate::ir::{self, types};

impl AsmEmitter {
    /// 関数を呼び出す情報がある物を受け取りアセンブリ言語を生成する
    ///
    /// `gen/asm_emitter.rs`の`extract_operand_text`から、関数呼び出しの
    /// 結果を値として使う(戻り値を任意のレジスタへ代入する)際にも
    /// 使われるため`pub(super)`にしている
    pub(super) fn emit_call_func(
        &mut self,
        meta_data: &inst::CallFuncMetaData,
        expand_struct_return: bool,
    ) -> String {
        // 生成するアセンブリコード
        let mut call_func = String::new();

        for (index, param) in meta_data
            .params
            .iter()
            .enumerate() 
        {
            // 引数の実際の型のサイズ
            // (レジスタの取得だけでなく、後段のニーモニックの
            //  サイズ決定にも同じサイズを使う必要がある)
            let param_ty = self.curr_inst[*param]
                .get_param_ty()
                .unwrap();
            // 引数のレジスタを取得
            let param_reg = self.asm_fmt.get_fmt_param::<String>(
                &index, 
                param_ty.clone()
            );
            // 引数のレジスタと値のidを挿入
            let src1_idx = if let Some(struct_idx) = self
                .resolve_struct_idx(param) 
            {
                struct_idx
            } else {
                *param
            };

            let opcode = if expand_struct_return && index == 0 {
                "address"
            } else if self.curr_inst[*param].is_pointer() {
                "address"
            } else {
                "mov"
            };

            let resize_size = if opcode == "address" {
                Size::DQ
            } else {
                param_ty.clone()
            };

            let src1_text = self.extract_operand_text(
                &src1_idx, 
                &resize_size.clone().wrap_dst_size()
            );
            let src1_text = self.asm_fmt
                .resize_reg_operand(&src1_text, &resize_size);

            let mut asm = self.asm_fmt
                .get_opcode_tmpl(opcode)
                .replace("{dst}", &param_reg)
                .replace("{src1}", &src1_text);
            asm = if self.check_node_is_mem_val(param).is_some() {
                self.asm_fmt
                    .fmt_memory_mnemonic_resize(opcode, &asm, &resize_size)
            } else {
                self.asm_fmt
                    .fmt_mnemonic_resize(opcode, &asm, &resize_size)
            };
            call_func.push_str(&asm);
        }
        let fn_label = if meta_data.temp_ty.is_empty() {
            emit_fn_name_id(&meta_data.name)
        } else {
            let generic_args = meta_data
                .temp_ty
                .iter()
                .map(|ty| format!("{:?}", ty))
                .collect::<Vec<_>>();
            emit_generic_fn_name_id(&meta_data.name, &generic_args)
        };
        call_func.push_str(
            &self
                .asm_fmt
                .get_call_func_fmt(
                    &fn_label
                )
            );
        call_func
    }

    /// 関数のメタ情報のbodyのデータを元に
    /// アセンブリ言語をフォーマットに沿って生成する関数
    ///
    /// ## 引数
    /// - func_meta_data
    /// - asm_fmt_name
    ///     出力するアセンブリ言語のフォーマットの名前
    pub(super) fn build_fn_process(
        &mut self,
        fn_meta_data: &mut (String, def_tree::FnDefInfo),
        asm_fmt_name: &Option<String>,
    ) {
        let this_is_self = fn_meta_data.1.first_param_is_self();
        let fn_ret_ty: SelfPtrInfo = fn_meta_data.1.get_ret_ty();
        println!("{:?}", this_is_self);
        // 新しく関数の作成、
        let fn_label = if fn_meta_data.1.temp_ty.is_empty() {
            emit_fn_name_id(&fn_meta_data.1.name)
        } else {
            let generic_args = fn_meta_data
                .1
                .temp_ty
                .iter()
                .map(|ty| format!("{:?}", ty))
                .collect::<Vec<_>>();
            emit_generic_fn_name_id(&fn_meta_data.1.name, &generic_args)
        };
        self.asm_text
            .push_str(
                &format!(
                    "{}:\n", 
                    &fn_label
                )
            );
        // 関数ごとにスタックの使用量をリセットする
        // (前の関数の`stk_use_counter`を持ち越すと、この関数の
        //  ローカル変数のオフセットが正しく計算できない)
        self.stk_use_counter = 0;
        // 生成済みの関数呼び出しの記録も、関数ごとにリセットする
        self.emitted_calls.clear();
        if fn_meta_data.1.stk_size != 0 {
            // 予約されたサイズ分確保する
            self.asm_text
                .push_str(
                    &self.asm_fmt
                        .gen_stack_frame(
                            fn_meta_data.1.stk_size
                        )
                    );
        } else {
            if &fn_meta_data.0 != "_start" {
                self.asm_text
                    .push_str(
                        self.asm_fmt
                            .func_frame_fmt()
                            .as_str()
                        );
            }
        }

        self.curr_inst = mem::take(&mut fn_meta_data.1.body);
        let returned_struct_idx = if fn_ret_ty.is_none() {
            self.curr_inst.iter().find_map(|node| match node {
                inst::Inst::Ret(idx) => self.resolve_struct_idx(&idx),
                _ => None,
            })
        } else {
            None
        };

        for (node_idx, node) in self.curr_inst
            .clone()
            .iter()
            .enumerate() 
        {
            match &node {
                inst::Inst::ExpectJmp(name) => {
                    // build_fn_proc.rs
                    self.expect_jmp(name);
                }
                inst::Inst::Str { dst, value } => {
                    // build_fn_proc.rs
                    self.gen_str_asm(dst, value);
                }
                inst::Inst::Expr(expr) => {
                    // build_fn_proc.rs
                    self.gen_expr_asm(expr);
                }
                inst::Inst::Num { .. } => {}
                inst::Inst::InitArr(_) => {}
                inst::Inst::Block(name) => {
                    self.asm_text.push_str(&format!("{}:\n", name));
                }
                inst::Inst::Comple { name, lines } => {
                    // build_fn_proc.rs
                    self.gen_complie_asm(name, lines, asm_fmt_name);
                }
                inst::Inst::Jmp(name) => {
                    // build_fn_proc.rs
                    self.asm_text.push_str(&format!("jmp {}\n", name));
                }
                inst::Inst::AssignVar { name, dst, value } => {
                    self.gen_assign_var_asm(name, dst, value, &this_is_self);
                }
                inst::Inst::Ret(idx) => {
                    // build_fn_proc.rs
                    self.gen_ret_asm(&fn_ret_ty, &idx);
                }
                inst::Inst::Mov {
                    name,
                    size,
                    dst,
                    src,
                } => {
                    let is_returned_struct = matches!(
                        size, 
                        types::Size::Struct(..)
                    )
                    && returned_struct_idx == self.resolve_struct_idx(dst);
                    if !is_returned_struct {
                        self.mov_value_ir(
                            size, 
                            dst, 
                            src, 
                            &name, 
                            &Some(size.clone())
                        );
                    }
                } // メモリに配置されている値の生成
                inst::Inst::MemoryValue(mem_value) => {
                    // call_func/mem_ir.rs
                    self.mem_val_ir(mem_value);
                }
                inst::Inst::Param(param) => {
                    let ty = node.get_param_ty().unwrap();
                    // 引数に使うレジスタを取得する
                    let reg_num = self.asm_fmt
                        .get_fmt_param::<usize>(&param.num, ty.clone());
                    self.insert_var_info(
                        &param.name,
                        asm_emitter::VarIndexInfo::new(
                            &reg_num, 
                            &ty, 
                            &param.dst
                        ),
                    );
                }
                inst::Inst::CallFunc(meta_data) => {
                    // 関数を呼ぶノードが変数に戻り値を代入しないばあいのみ生成
                    if meta_data.parent == ir::IS_NOT_ASSIGN_EXPR {
                        let asm_text = self.emit_call_func(&meta_data, false);
                        self.asm_text.push_str(asm_text.as_str());
                        // 値として参照された場合(`a: int = func(10)`など)に
                        // 戻り値のレジスタを返せるよう、生成済みとして記録する
                        self.emitted_calls.push(node_idx);
                    }
                }
                /*inst::Inst::Struct { mem, is_self, .. } => {
                    if *is_self {
                        let ini_struct_asm = self.emit_struct_ini_asm(mem.to_vec(), this_is_self);
                        self.asm_text.push_str(&ini_struct_asm);
                    }
                }*/
                _ => {},
            }
        }
        // retがない場合つけたす
        if !self.asm_text.ends_with("ret\n") {
            self.asm_text.push_str("leave\nret\n");
        }
    }
}




impl AsmEmitter {
    #[inline(always)]
    pub(super) fn expect_jmp(
        &mut self, 
        name: &String
    ) {
        // 次のフォーマットに使うラベルの名前を予約する
        if self.reserved_label_name.is_none() {
            self.reserved_label_name = Some(name.to_string());
        } else {
            // 予約するラベルの名前は予約するときにNoneで無ければいけない
            panic!("system err");
        }
    }

    #[inline(always)]
    pub(super) fn gen_str_asm(
        &mut self, 
        dst: &usize, 
        value: &String
    ) {
        let label_name = format!("M{}", self.data_idx.to_string());
        let fmt_data = self.asm_fmt.get_str_fmt(&value, &label_name);
        self.data_sec_text.push_str(&fmt_data);
        self.data_map.push((*dst, label_name));
        self.data_idx += 1;
    }

    #[inline(always)]
    pub(super) fn gen_expr_asm(
        &mut self, 
        expr: &inst::ExprInst
    ) {
        let asm = self.format_expr_inst(&expr);

        self.asm_text.push_str(&asm);

        // 式の結果を置いたレジスタを使用中として記録する
        self.used_reg.mark_used(&self.reg_idx);
        self.last_inst_idx.push((expr.dst, self.reg_idx));
        self.reg_idx += 1;
    }

    #[inline(always)]
    pub(super) fn gen_complie_asm(
        &mut self, 
        name: &String, 
        lines: &Vec<(String, Vec<usize>)>, 
        asm_fmt_name: &Option<String>
    ) {
        if let Some(asm_name) = asm_fmt_name {
            if name.as_str() == asm_name.as_str() {
                self.deploy_inline_asm(&name, &lines);
            } else {
                panic!("unmatch asm name");
            }
        } else {
            self.deploy_inline_asm(&name, &lines);
        }
    }

    #[inline(always)]
    pub(super) fn gen_assign_var_asm(
        &mut self,
        name: &String,
        dst: &usize,
        value: &usize,
        this_is_self: &bool,
    ) {
        let is_mem_write = matches!(
            self.curr_inst[*dst],
            inst::Inst::Pointer(..)
                | inst::Inst::InsertArr { .. }
                | inst::Inst::RefStruct { .. }
        );
        
        if is_mem_write {
            self.write_mem(
                name, 
                &dst, 
                &value, 
                &Some(self.get_var_ty(&name))
            );
        } else {
            // 通常の変数への再代入(`b = 10`など)
            self.update_value_info(&name, &value);

            let current_reg = self.reg_idx;
            let s: SelfPtrInfo = if *this_is_self {
                None
            } else {
                self.get_var_ty(&name).wrap_dst_size()
            };

            // 代入先の変数の型(サイズ)を確認し、ポインタ型
            // であれば、専用のフォーマット(`get_ptr`)で
            // アドレスのオペランドを組み立てる
            let text = if self.get_var_ty(&name)
                .is_pointer()
                .is_some() 
            {
                self.assign_val_ty_is_ptr(
                    current_reg, 
                    value, 
                    &s
                )
            } else {
                self.assign_val_is_not_ptr(
                    current_reg, 
                    value, 
                    &s
                )
            };

            if self.expr_vars
                .iter()
                .find(|v| v == &name)
                .is_some() 
            {
                self.update_value_reg(
                    &name, 
                    &current_reg
                );
            }
            self.asm_text.push_str(&text);
        }
    }

    #[inline(always)]
    pub(super) fn gen_ret_asm(
        &mut self, 
        fn_ret_ty: &SelfPtrInfo,
        idx: &usize,
    ) {
        if fn_ret_ty.is_none() {
            if let Some(struct_idx) = self.resolve_struct_idx(idx) {
                let mem = match self.curr_inst[struct_idx].clone() {
                    inst::Inst::Struct { mem, .. } => mem,
                    _ => panic!("構造体の戻り値を解決できません"),
                };
                let t = self.emit_struct_ini_asm(
                    mem, 
                    true
                );
                self.asm_text.push_str(
                    format!(
                        "{}{}", 
                        t.as_str(),
                        self.asm_fmt
                            .func_frame_end()
                            .as_str()
                    ).as_str()
                );
                self.asm_text.push_str("ret\n");
                return;
            }
        }

        let ret_asm = self.format_line(
            "mov", 
            Some(&0), 
            &idx, 
            None, 
            &fn_ret_ty
        );
        self.asm_text.push_str(&ret_asm);
        self.asm_text
            .push_str(
                self.asm_fmt
                    .func_frame_end()
                    .as_str()
                );
        self.asm_text.push_str("ret\n");
    }
}