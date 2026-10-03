use super::*;
use crate::compiler::ir::types;

impl AsmEmitter {
    pub(super) 
    fn mov_value_ir(
        &mut self,
        size: &types::Size,
        dst: usize,
        src: usize,
        name: &Option<String>,
        this_is_self: &SelfPtrInfo,
    ) {
        // 経由の間接参照になってしまっていた。
        if let types::Size::Struct(_) = size {
            // コンストラクタ呼び出しで初期化する構造体は、暗黙の`self`
            // 引数(`GetAddress(GetPtr)`)が指すスタック領域を、先に
            // `stk_use_counter`から確保する。IRの`stk`のままだと、先に
            // 確保済みの配列などと領域が重なってしまう
            if let inst::Inst::CallFunc(meta_data) = self.curr_inst[src].clone() {
                if let Some(&self_arg) = meta_data.params.get(0) {
                    if let inst::Inst::GetAddress(mem_idx) = self.curr_inst[self_arg].clone() {
                        if matches!(self.curr_inst[mem_idx], inst::Inst::GetPtr { .. }) {
                            self.alloc_struct_stk(mem_idx, size.to_bytes());
                        }
                    }
                }
            }
            // 呼び出し自体(`lea`+`call`)は副作用として`self.asm_text`へ
            // 積まれる。戻り値のオペランド文字列自体は構造体には
            // 使えないので捨てる
            let _ = self.extract_operand_text(src, &this_is_self);
            if self.struct_mem(name, size, src, dst).is_none() {
                return ();
            }

            let inst::Inst::CallFunc(meta_data) = self.curr_inst[src].clone() else {
                panic!(
                    "構造体を返す初期化式はコンストラクタ呼び出しである必要があります: {:?}",
                    self.curr_inst[src]
                );
            };
            let self_arg_idx = *meta_data
                .params
                .get(0)
                .expect("構造体を返す関数は暗黙のselfポインタ引数を持つ必要があります");
            let inst::Inst::GetAddress(mem_idx) = self.curr_inst[self_arg_idx].clone() else {
                panic!("システムエラー: 暗黙のselfポインタ引数がGetAddressではありません");
            };
            let inst::Inst::GetPtr { stk, .. } = self.curr_inst[mem_idx].clone() else {
                panic!("システムエラー: 暗黙のselfポインタ引数の参照先がGetPtrではありません");
            };
            let stk = self.ptr_stk(mem_idx, stk);

            if let Some(var_name) = name {
                self.insert_var_info(
                    var_name,
                    asm_emitter::VarIndexInfo::new_stack(stk, &size, dst),
                );
            }
            return;
        }

        if size.is_pointer().is_none() && self.data_map.iter().find(|v| v.0 == src).is_some() {
            // 子のノードがstatic領域の値で、かつ宣言先の型がポインタで
            // ない場合のみ、変数名だけを登録する(この場合は変数の
            // 実体が静的領域そのものであり、レジスタへ値をロード
            // する必要がないため)。
            self.insert_var_info(
                &name.as_ref().unwrap(),
                asm_emitter::VarIndexInfo::new(self.reg_idx, &size, dst),
            );
        } else {
            // レジスタに置く変数
            // 使用中のレジスタ(引数など)と`%rax`/`%rdx`を避けて確保する
            let reg = self.alloc_reg();
            self.reg_idx = reg;
            // このレジスタを使用中として記録する
            self.used_reg.mark_used(reg);
            // ポインタ型の変数へ数値リテラル(`ptr: int* = 0`の
            // ような`0`=NULL初期化など)を代入する場合は、
            // アドレスを求める`lea`(="address")命令ではなく、
            // ポインタのサイズ(64bit)に合わせた`movq`で
            // そのまま即値をレジスタへ書き込む
            let is_literal_num = matches!(self.curr_inst[src], inst::Inst::Num { .. });

            let formated = if size.is_pointer().is_some() && is_literal_num {
                let dst_reg = self.asm_fmt.get_fmt_reg(reg, &Size::DQ);
                let value_operand = self.extract_operand_text(src, &this_is_self);
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
                self.format_line(mnemonic, Some(reg), src, None, this_is_self)
            };

            self.asm_text.push_str(&formated);
            // 初期値が式の結果だった場合、そのレジスタはもう不要
            self.release_expr_temp(src);

            if let Some(var_name) = name {
                self.insert_var_info(
                    &var_name,
                    asm_emitter::VarIndexInfo::new(self.reg_idx, &size, dst),
                );
                let current_reg = self.reg_idx;

                if self
                    .expr_vars
                    .iter()
                    .find(|v| v.as_str() == var_name.as_str())
                    .is_some()
                {
                    self.update_value_reg(&var_name, current_reg);
                }
            }
        }
    }

    pub(super) fn mem_val_ir(&mut self, mem_value: &inst::MemoryInst) {
        match mem_value {
            inst::MemoryInst::Memory {
                name,
                size,
                src,
                kind,
                dst,
            } => {
                let dst_size: SelfPtrInfo = size.wrap_dst_size();

                if kind == &inst::MemoryKind::Static {
                    self.is_static_var(src, *dst, name, &size.wrap_dst_size());
                } else {
                    let base = match &self.curr_inst[*dst] {
                        inst::Inst::Pointer(..)
                        | inst::Inst::Param(..)
                        | inst::Inst::GetPtr { .. } => self.extract_operand_text(*dst, &dst_size),
                        _ => "%rbp".to_string(),
                    };

                    let mut txt = String::new();
                    // 要素0が最も%rbpに近い位置(`-4(%rbp)`)に来て、要素が
                    // 増えるほどアドレスが小さくなる(`-4`, `-8`, `-12`, ...)。
                    // 領域全体(`要素のサイズ * 要素数`)を先に確保し、
                    // 要素`k`のオフセットは `arr_base + (k + 1) * 要素のサイズ`
                    let elem_bytes = size.to_bytes();
                    let arr_base = self.stk_use_counter;
                    self.stk_use_counter = arr_base + elem_bytes * src.len();
                    for (k, idx) in src.iter().enumerate() {
                        let value = self.extract_operand_text(*idx, &dst_size);
                        let s = &self
                            .asm_fmt
                            .fmt_ref_operand(&base, arr_base + (k + 1) * elem_bytes);

                        let mov_line = self
                            .asm_fmt
                            .get_opcode_tmpl("mov")
                            .replace("{dst}", &s)
                            .replace("{src1}", value.as_str());
                        let store_size = if size.is_pointer().is_some() {
                            Size::DQ
                        } else {
                            size.clone()
                        };
                        txt.push_str(
                            self.asm_fmt
                                .fmt_memory_mnemonic_resize("mov", &mov_line, &store_size)
                                .as_str(),
                        );
                    }
                    self.insert_var_info(
                        &name,
                        asm_emitter::VarIndexInfo::new(self.reg_idx, &size, *dst)
                            .with_arr_base(arr_base),
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
        dst: usize,
        name: &String,
        this_is_self: &SelfPtrInfo,
    ) {
        println!("src/gen/call_func/MemoryValue");
        let val = self.extract_operand_text(*src.last().unwrap(), &this_is_self);
        let label_name = format!("M{}", self.data_idx.to_string());
        let fmt_data =
            self.asm_fmt
                .get_static_num_fmt(&val, &label_name, this_is_self.as_ref().unwrap());
        self.data_sec_text.push_str(&fmt_data);
        self.data_map.push((dst, label_name));
        self.data_idx += 1;
        // 子のノードがstaticりょいきの値なので、変数名だけ登録する
        self.insert_var_info(
            &name,
            asm_emitter::VarIndexInfo::new(self.reg_idx, this_is_self.as_ref().unwrap(), dst),
        );
    }

    fn struct_mem(
        &mut self,
        name: &Option<String>,
        size: &types::Size,
        src: usize,
        dst: usize,
    ) -> Option<()> {
        if let inst::Inst::Struct { mem, is_self, .. } = self.curr_inst[src].clone() {
            // `emit_struct_ini_asm`はメンバーを書き込みながら
            // `self.stk_use_counter`を進めていくため、呼び出し後
            // では構造体自身の先頭オフセット(`%rbp`から見た位置)が
            // 分からなくなってしまう。呼び出し前の値を控えておき、
            // それをこの変数の実体の位置として登録する
            // 構造体のポインタは先頭のメンバーを指すので、`alloc_struct_stk`と
            // 同様に、先頭のメンバーがはみ出す分(8byte)を余分に確保する
            let struct_stk_offset = if is_self {
                self.stk_use_counter
            } else {
                let base = self.stk_use_counter.div_ceil(8) * 8 + 8;
                self.stk_use_counter = base;
                base
            };
            let ini_asm = self.emit_struct_ini_asm(mem, is_self);
            if !is_self {
                self.stk_use_counter = self.stk_use_counter.max(struct_stk_offset + size.to_bytes());
            }
            self.asm_text.push_str(ini_asm.as_str());
            if let Some(var_name) = name {
                self.insert_var_info(
                    var_name,
                    asm_emitter::VarIndexInfo::new_stack(struct_stk_offset, &size, dst),
                );
            }
            return None;
        }
        Some(())
    }
}
