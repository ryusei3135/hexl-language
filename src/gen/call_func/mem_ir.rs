use super::*;


impl AsmEmitter {
    pub fn gen_mem_value(
        &mut self,
        mem_value: &inst::MemoryInst,
        this_is_self: bool,
    ) {
        match mem_value {
            inst::MemoryInst::Memory { name, size, src, kind, dst } => {
                if kind == &inst::MemoryKind::Static {
                    self.is_static_var(src, size, &dst, name, this_is_self);
                } else {
                    // スタック領域のローカル変数を作成する
                    //
                    // `dst`が新しく確保するローカル変数のスロット
                    // ではなく、`self`(メソッドの第一引数として
                    // 渡されるポインタ、`%rdi`など)や`[p]`のような
                    // 既存のメモリを指している場合(例:
                    // `ret Self { c: {0, 1, 2} }`のように構造体
                    // リテラルの配列フィールドを構築する場合)は、
                    // 独立したローカルの一時領域(`%rbp`基準)を
                    // 新しく確保するのではなく、そのメモリへ
                    // 直接書き込む必要がある。
                    // 以前はここで`dst`を全く見ずに常に`%rbp`
                    // 決め打ちでオペランドを組み立てていたため、
                    // このようなケースでも誤ってローカルの
                    // 一時領域に書き込まれてしまっていた。
                    let base = match &self.curr_inst[*dst] {
                        inst::Inst::Pointer(..)
                        | inst::Inst::Param(..)
                        | inst::Inst::GetPtr { .. } => {
                            self.extract_operand_text(&dst, this_is_self)
                        }
                        _ => "%rbp".to_string(),
                    };

                    let mut txt = String::new();
                    for idx in src.iter() {
                        let value = self
                            .extract_operand_text(
                                &idx, 
                                this_is_self
                            );
                        // スタックの場所を更新
                        // (この変数のオフセットは、これまで使用した
                        //  スタックのサイズ`stk_use_counter`に、
                        //  この変数のサイズを足したもの)
                        self.stk_use_counter += size.to_bytes();
                        let s = &self.asm_fmt.fmt_ref_operand(
                            &base,
                            &self.stk_use_counter,
                        );

                        let mov_line = self.asm_fmt
                        .get_opcode_tmpl("mov")
                        .replace("{dst}", &s)
                        .replace("{src1}", value.as_str());
                        txt.push_str(
                            self.asm_fmt
                            .fmt_mnemonic_resize(
                                "mov", 
                                &mov_line, 
                                &size
                            )
                            .as_str()
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

    fn is_static_var(
        &mut self,
        src: &Vec<usize>,
        size: &ir::types::Size,
        dst: &usize,
        name: &String,
        this_is_self: bool,
    ) {
        println!("src/gen/call_func/MemoryValue");
        let value = self.extract_operand_text(
            &src.last().unwrap(), 
            this_is_self
        );
        let label_name = format!("M{}", self.data_idx.to_string());
        let fmt_data = self.asm_fmt
            .get_static_num_fmt(
                &value, 
                &label_name, 
                size
            );
        self.data_sec_text.push_str(&fmt_data);
        self.data_map.push((*dst, label_name));
        self.data_idx += 1;
        // 子のノードがstaticりょいきの値なので、変数名だけ登録する
        self.insert_var_info(
            &name,
            asm_emitter::VarIndexInfo::new(&self.reg_idx, &size, dst)
        );
    }
}
