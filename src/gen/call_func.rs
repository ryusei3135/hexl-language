use super::*;
use crate::ir;


impl AsmEmitter {
    /// 関数を呼び出す情報がある物を受け取りアセンブリ言語を生成する
    ///
    /// `gen/asm_emitter.rs`の`extract_operand_text`から、関数呼び出しの
    /// 結果を値として使う(戻り値を任意のレジスタへ代入する)際にも
    /// 使われるため`pub(super)`にしている
    pub(super) fn emit_call_func(
        &mut self,
        meta_data: &inst::CallFuncMetaData
    ) -> String {
        // 生成するアセンブリコード
        let mut call_func = String::new();

        for (index, param) in meta_data.params.iter().enumerate() {
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
    /// - func_meta_data
    /// - asm_fmt_name
    ///     出力するアセンブリ言語のフォーマットの名前
    pub(super) fn build_func_process(
        &mut self,
        func_meta_data: &mut (String, def_tree::FuncDefInfo),
        asm_fmt_name: &Option<String>,
    ) {
        // 新しく関数の作成、
        self.asm_text.push_str(&format!("{}:\n", &func_meta_data.0));
        // 関数ごとにスタックの使用量をリセットする
        // (前の関数の`stk_use_counter`を持ち越すと、この関数の
        //  ローカル変数のオフセットが正しく計算できない)
        self.stk_use_counter = 0;
        if func_meta_data.1.stk_size != 0 {
            // 予約されたサイズ分確保する
            self.asm_text.push_str(&self.asm_fmt.gen_stack_frame(func_meta_data.1.stk_size));
        } else {
            if &func_meta_data.0 != "_start" {
                self.asm_text.push_str(self.asm_fmt.func_frame_fmt().as_str());
            }
        }

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

                    // 式の結果を置いたレジスタを使用中として記録する
                    self.used_reg.mark_used(&self.reg_idx);
                    self.last_inst_idx.push(
                        (expr.dst, self.reg_idx)
                    );
                    self.reg_idx += 1;
                }
                inst::Inst::Num{ .. } => {
                }
                inst::Inst::InitArr(_) => {
                },
                inst::Inst::Block(name) => {
                    self.asm_text.push_str(&format!("{}:\n", name));
                }
                inst::Inst::Comple{name, lines} => {
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
                inst::Inst::Jmp(name) => {
                    self.asm_text.push_str(&format!("jmp {}\n", name));
                }
                inst::Inst::AssignVar { name, dst, value } => {
                    // 代入先(`dst`)が「変数そのもの」ではなく、
                    // 実際に書き込むべきメモリを表すノードを
                    // 参照している場合は、変数への再代入ではなく
                    // そのメモリへ直接値を書き込む必要がある
                    // - `Inst::Pointer`: `[b] = 20`(ポインタの参照先)
                    // - `Inst::InsertArr`: `[arr 0] = 10`(配列の要素)
                    let is_mem_write = matches!(
                        self.curr_inst[*dst],
                        inst::Inst::Pointer(..) | inst::Inst::InsertArr { .. }
                    );

                    if is_mem_write {
                        // 書き込み先のメモリのオペランド
                        let dst_operand = self.extract_operand_text(dst);
                        // 書き込む値のオペランド
                        let value_operand = self.extract_operand_text(value);

                        let mut text = self.asm_fmt
                            .get_opcode_tmpl("mov")
                            .replace("{dst}", &dst_operand)
                            .replace("{src1}", &value_operand);

                        text = self.asm_fmt.fmt_mnemonic_resize("mov", &text, &self.get_var_ty(&name));
                        self.asm_text.push_str(&text);
                    } else {
                        // 通常の変数への再代入(`b = 10`など)
                        self.update_value_info(&name, &value);

                        let current_reg = self.reg_idx;
                        // 代入する値(value)がアドレスを求める式
                        // (`GetAddress`/`Pointer`)の場合のみ`lea`相当の
                        // ニーモニックを使う。
                        // (代入先の変数がかつてポインタとして定義された
                        //  ものであっても、今回代入する値自体が
                        //  アドレス計算を必要としないなら`mov`で良い)
                        let mnemonic = if self.curr_inst[*value].is_pointer() {
                            "address"
                        } else {
                            "mov"
                        };

                        let text = self.format_line(mnemonic, Some(&current_reg), &value, None);

                        if self.expr_vars.iter().find(|v| v == &name).is_some() {
                            self.update_value_reg(&name, &current_reg);
                        }
                        self.asm_text.push_str(&text);
                    }
                }
                inst::Inst::Ret(idx) => {
                    let ret_asm = self.format_line(
                        "mov",
                        Some(&0),
                        &idx,
                        None
                    );
                    self.asm_text.push_str(&ret_asm);
                    self.asm_text.push_str(self.asm_fmt.func_frame_end().as_str());
                    self.asm_text.push_str("ret\n");
                }
                inst::Inst::Mov { name, size, dst, src } => {
                    if self.data_map.iter().find(|v| &v.0 == src).is_some() {
                        // 子のノードがstaticりょいきの値なので、変数名だけ登録する
                        self.insert_var_info(
                            &name.as_ref().unwrap(),
                            asm_emitter::VarIndexInfo::new(&self.reg_idx, &size, dst)
                        );
                    } else {
                        self.reg_idx += 1;
                        // レジスタに置く変数
                        let reg = self.reg_idx.clone();
                        // このレジスタを使用中として記録する
                        self.used_reg.mark_used(&reg);
                        // メモリのポインタか、値かで、ニーモニックが変わる
                        let mnemonic = if size.is_pointer().is_some() {
                            // ポインタの場合
                            "address"
                        } else {
                            // 普通の場合
                            "mov"
                        };
                        let formated = self.format_line(mnemonic, Some(&reg), &src, None);

                        self.asm_text.push_str(&formated);

                        if let Some(var_name) = name {
                            self.insert_var_info(
                                &var_name, 
                                asm_emitter::VarIndexInfo::new(&self.reg_idx, &size, dst)
                            );
                            let current_reg = self.reg_idx;

                            if self.expr_vars.iter().find(|v| v.as_str() == var_name.as_str()).is_some() {
                                self.update_value_reg(&var_name, &current_reg);
                            }
                        }
                    }
                }
                // メモリに配置されている値の生成
                inst::Inst::MemoryValue(memory_value) => {
                    match memory_value {
                        inst::MemoryInst::Memory { name, size, src, kind, dst } => {
                            if kind == &inst::MemoryKind::Static {
                                println!("src/gen/call_func/MemoryValue");
                                let value = self.extract_operand_text(&src.last().unwrap());
                                let label_name = format!("M{}", self.data_idx.to_string());
                                let fmt_data = self.asm_fmt.get_static_num_fmt(&value, &label_name, &size);
                                self.data_sec_text.push_str(&fmt_data);
                                self.data_map.push((*dst, label_name));
                                self.data_idx += 1;
                                // 子のノードがstaticりょいきの値なので、変数名だけ登録する
                                self.insert_var_info(
                                    &name,
                                    asm_emitter::VarIndexInfo::new(&self.reg_idx, &size, dst)
                                );
                            } else {
                                // スタック領域のローカル変数を作成する 
                                let mut txt = String::new();
                                for idx in src.iter() {
                                    let value = self.extract_operand_text(&idx);
                                    // スタックの場所を更新
                                    // (この変数のオフセットは、これまで使用した
                                    //  スタックのサイズ`stk_use_counter`に、
                                    //  この変数のサイズを足したもの)
                                    self.stk_use_counter += size.to_bytes();
                                    let s = &self.asm_fmt.fmt_ref_operand(
                                        &"%rbp".to_string(),
                                        &self.stk_use_counter,
                                    );

                                    let mov_line = self.asm_fmt
                                    .get_opcode_tmpl("mov")
                                    .replace("{dst}", &s)
                                    .replace("{src1}", value.as_str());
                                    txt.push_str(
                                        self.asm_fmt.fmt_mnemonic_resize("mov", &mov_line, &size).as_str()
                                    );
                                }
                                self.insert_var_info(
                                    &name, 
                                    asm_emitter::VarIndexInfo::new(&self.reg_idx, &size, dst)
                                );
                                self.asm_text.push_str(txt.as_str());
                            }
                        }
                        _ => panic!(),
                    }
                }
                inst::Inst::Param(param) => {
                    // 引数に使うレジスタを取得する 
                    let reg_num = self.asm_fmt.get_fmt_param::<usize>(&param.num);
                    self.insert_var_info(
                        &param.name,
                        asm_emitter::VarIndexInfo::new(&reg_num, &Size::DQ, &param.dst)
                    );
                }
                inst::Inst::CallFunc(meta_data) => {
                    // 関数を呼ぶノードが変数に戻り値を代入しないばあいのみ生成
                    if meta_data.parent == ir::IS_NOT_ASSIGN_EXPR {
                        let asm_text = self.emit_call_func(&meta_data);
                        self.asm_text.push_str(asm_text.as_str());
                    }
                }
                t => println!("call func >> gen {:?}", t),
            }
        }
        // retがない場合つけたす
        if !self.asm_text.ends_with("ret\n") {
            self.asm_text.push_str("leave\nret\n");
        }
    }
}
