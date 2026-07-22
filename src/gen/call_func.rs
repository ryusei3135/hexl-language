use super::*;


impl AsmEmitter {
    pub(super) fn emit_call_func(
        &mut self,
        meta_data: &inst::CallFuncMetaData
    ) -> String {
        // 生成するアセンブリコード
        let mut call_func = String::new();
        println!("{:?}", meta_data);

        for (index, param) in meta_data.params.iter().enumerate() {
            println!(">> {:?}", param);
            // 引数のレジスタを取得
            let param_reg = self.asm_fmt.get_fmt_param::<usize>(&index);
            // 引数のレジスタと値のidを挿入
            let param_asm = self.format_line("mov", Some(&param_reg), &param, None);
            call_func.push_str(&param_asm);
        }
        call_func.push_str(&self.asm_fmt.get_call_func_fmt(&meta_data.name));
        call_func
    }

    /// 関数のメタ情報のbodyのデータを元に
    /// アセンブリ言語をフォーマットに沿って生成する関数
    ///
    /// ## 引数
    /// - func_body
    ///     関数の処理
    /// - func_name
    ///     関数の名前
    /// - asm_fmt_name
    ///     出力するアセンブリ言語のフォーマットの名前
    pub(super) fn build_func_process(
        &mut self,
        func_meta_data: &mut (String, builder::FuncDefInfo),
        asm_fmt_name: &Option<String>,
    ) {
        // 新しく関数の作成、
        self.asm_text.push_str(&format!("{}:\n", &func_meta_data.0));
        self.curr_inst = mem::take(&mut func_meta_data.1.body);

        for node in self.curr_inst.clone().iter() {
            match &node {
                inst::Inst::ExpectJmp(name) => {
                    // 次のフォーマットに使うラベルの名前を予約する
                    if self.reserved_label_name.is_none() {
                        self.reserved_label_name = Some(name.to_string());
                    } else {
                        // 予約するラベルの名前は予約するときにNoneで無ければいけない
                        panic!("system err");
                    }
                }
                inst::Inst::Str { dst, value } => {
                    let label_name = format!("M{}", self.data_idx.to_string());
                    let fmt_data = self.asm_fmt.get_str_fmt(&value, &label_name);
                    self.data_sec_text.push_str(&fmt_data);
                    self.data_map.push((*dst, label_name));
                    self.data_idx += 1;
                }
                inst::Inst::Expr(expr) => {
                    let asm = self.format_expr_inst(&expr);

                    self.asm_text.push_str(&asm);

                    self.last_inst_idx.push(
                        (expr.dst, self.reg_idx)
                    );
                    self.reg_idx += 1;
                }
                inst::Inst::Num{ .. } => {
                }
                inst::Inst::Block(name) => {
                    self.asm_text.push_str(&format!("{}:\n", name));
                }
                inst::Inst::Comple{name, nodes} => {
                    if let Some(asm_name) = asm_fmt_name {
                        if name.as_str() == asm_name.as_str() {
                            self.deploy_inline_asm(&name, &nodes);
                        } else {
                            panic!("unmatch asm name");
                        }
                    } else {
                        self.deploy_inline_asm(&name, &nodes);
                    }
                }
                inst::Inst::Jmp(name) => {
                    self.asm_text.push_str(&format!("jmp {}\n", name));
                }
                inst::Inst::AssignVar { name, value } => {
                    self.update_value_info(&name, &value);

                    let current_reg = self.reg_idx;

                    if self.expr_vars.iter().find(|v| v.as_str() == name.as_str()).is_some() {
                        self.update_value_reg(&name, &current_reg);
                    }
                }
                inst::Inst::Ret(idx) => {
                    self.format_line(
                        "mov",
                        Some(&0),
                        &idx,
                        None
                    );
                    self.asm_text.push_str("ret\n");
                }
                inst::Inst::Mov { name, dst, src } => {
                    if self.data_map.iter().find(|v| &v.0 == src).is_some() {
                        // 子のノードがstaticりょいきの値なので、変数名だけ登録する
                        self.insert_var_info(&name.as_ref().unwrap(), &dst, self.reg_idx);
                    } else {
                        // レジスタに置く変数
                        let formated = self.format_line("mov", Some(dst), src, None);

                        self.reg_idx += 1;
                        self.asm_text.push_str(&formated);

                        if let Some(var_name) = name {
                            self.insert_var_info(&var_name, &dst, self.reg_idx);
                            let current_reg = self.reg_idx;

                            if self.expr_vars.iter().find(|v| v.as_str() == var_name.as_str()).is_some() {
                                self.update_value_reg(&var_name, &current_reg);
                            }
                        }
                    }
                }
                inst::Inst::CallFunc(meta_data) => {
                    let asm_text = self.emit_call_func(&meta_data);
                    self.asm_text.push_str(asm_text.as_str());
                }
                inst::Inst::Param(param) => {
                    let reg_num = self.asm_fmt.get_fmt_param::<usize>(&param.num);
                    self.insert_var_info(
                        &param.name, &param.dst, reg_num
                    );
                }
                _ => {}
            }
        }
    }
}
