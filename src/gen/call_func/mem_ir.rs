use super::*;
use crate::ir::types;

impl AsmEmitter {
    pub(super) fn mov_value_ir(
        &mut self,
        size: &types::Size,
        dst: &usize,
        src: &usize,
        name: &Option<String>,
        this_is_self: bool,
    ) {
        if self.data_map.iter().find(|v| &v.0 == src).is_some() {
            // 子のノードがstaticりょいきの値なので、変数名だけ登録する
            self.insert_var_info(
                &name.as_ref().unwrap(),
                asm_emitter::VarIndexInfo::new(&self.reg_idx, &size, dst),
            );
        } else {
            self.reg_idx += 1;
            // レジスタに置く変数
            let reg = self.reg_idx.clone();
            // このレジスタを使用中として記録する
            self.used_reg.mark_used(&reg);
            // ポインタ型の変数へ数値リテラル(`ptr: int* = 0`の
            // ような`0`=NULL初期化など)を代入する場合は、
            // アドレスを求める`lea`(="address")命令ではなく、
            // ポインタのサイズ(64bit)に合わせた`movq`で
            // そのまま即値をレジスタへ書き込む
            let is_literal_num = matches!(self.curr_inst[*src], inst::Inst::Num { .. });

            let formated = if size.is_pointer().is_some() && is_literal_num {
                let dst_reg = self.asm_fmt.get_fmt_reg(&reg, &Size::DQ);
                let value_operand = self.extract_operand_text(&src, this_is_self);
                let text = self
                    .asm_fmt
                    .get_opcode_tmpl("mov")
                    .replace("{dst}", &dst_reg)
                    .replace("{src1}", &value_operand);
                self.asm_fmt.fmt_mnemonic_resize("mov", &text, &Size::DQ)
            } else {
                // メモリのポインタか、値かで、ニーモニックが変わる
                let mnemonic = if size.is_pointer().is_some() {
                    // ポインタの場合
                    "address"
                } else {
                    // 普通の場合
                    "mov"
                };
                self.format_line(mnemonic, Some(&reg), &src, None, this_is_self)
            };

            self.asm_text.push_str(&formated);

            if let Some(var_name) = name {
                self.insert_var_info(
                    &var_name,
                    asm_emitter::VarIndexInfo::new(&self.reg_idx, &size, dst),
                );
                let current_reg = self.reg_idx;

                if self
                    .expr_vars
                    .iter()
                    .find(|v| v.as_str() == var_name.as_str())
                    .is_some()
                {
                    self.update_value_reg(&var_name, &current_reg);
                }
            }
        }
    }

    pub(super) fn mem_value_ir(&mut self, mem_value: &inst::MemoryInst, this_is_self: bool) {
        match mem_value {
            inst::MemoryInst::Memory {
                name,
                size,
                src,
                kind,
                dst,
            } => {
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
                        let value = self.extract_operand_text(&idx, this_is_self);
                        // スタックの場所を更新
                        // (この変数のオフセットは、これまで使用した
                        //  スタックのサイズ`stk_use_counter`に、
                        //  この変数のサイズを足したもの)
                        self.stk_use_counter += size.to_bytes();
                        let s = &self.asm_fmt.fmt_ref_operand(&base, &self.stk_use_counter);

                        let mov_line = self
                            .asm_fmt
                            .get_opcode_tmpl("mov")
                            .replace("{dst}", &s)
                            .replace("{src1}", value.as_str());
                        txt.push_str(
                            self.asm_fmt
                                .fmt_mnemonic_resize("mov", &mov_line, &size)
                                .as_str(),
                        );
                    }
                    self.insert_var_info(
                        &name,
                        asm_emitter::VarIndexInfo::new(&self.reg_idx, &size, dst),
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
        let value = self.extract_operand_text(&src.last().unwrap(), this_is_self);
        let label_name = format!("M{}", self.data_idx.to_string());
        let fmt_data = self.asm_fmt.get_static_num_fmt(&value, &label_name, size);
        self.data_sec_text.push_str(&fmt_data);
        self.data_map.push((*dst, label_name));
        self.data_idx += 1;
        // 子のノードがstaticりょいきの値なので、変数名だけ登録する
        self.insert_var_info(
            &name,
            asm_emitter::VarIndexInfo::new(&self.reg_idx, &size, dst),
        );
    }
}
