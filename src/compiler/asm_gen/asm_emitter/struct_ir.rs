use super::*;

impl AsmEmitter {
    pub(in crate::compiler::asm_gen) 
    fn emit_struct_ini_asm(
        &mut self,
        struct_node: Vec<inst::MemoryInst>,
        this_is_self: bool,
    ) -> String {
        let mut struct_txt = String::new();
        let mut add_size = 0;
        for member in struct_node.clone().iter() {
            let inst::MemoryInst::Member {
                value_idx, size, ..
            } = member
            else {
                panic!();
            };

            let member_size = Some(size.clone());

            let value = self
                .extract_operand_text(*value_idx, &member_size)
                .to_string();
            if value.is_empty() {
                let inst::Inst::InitArr(arr) = self.curr_inst[*value_idx].clone() else {
                    panic!();
                };
                struct_txt.push_str(self.init_arr_txt::<true>(&arr, &member_size).as_str());
                continue;
            }
            // このメンバー分を足した「累積」サイズ
            // (これが、このメンバーの`%rbp`からのオフセットになる)
            add_size += size.to_bytes();

            // ポインタは先頭のメンバーを指す(先頭のメンバーのオフセットは0)ので、
            // このメンバーより前のメンバーの累積サイズがオフセットになる
            let member_off = add_size - size.to_bytes();
            let offset = if this_is_self {
                // `self`のポインタ先に直接書き込むので、既存の
                // `stk_use_counter`(このスコープでのスタック使用量)は
                // 無関係
                member_off
            } else {
                // 既に使用していたスタックのサイズ + 前のメンバーの累積サイズ
                self.stk_use_counter + member_off
            };

            let fmted = self.asm_fmt.get_fmt_struct_member(value, &size, offset);

            if this_is_self {
                // 第一引数(`self`のポインタ)のレジスタを取得し、
                // `%rbp`をそのレジスタに置き換える
                // (ポインタなので64bitのレジスタ(`Size::DQ`)を使う)
                let self_ptr_reg = &self.asm_fmt.get_fmt_param::<String>(0, &Size::DQ);
                struct_txt.push_str(&fmted.replace("%rbp", &self_ptr_reg));
            } else {
                struct_txt.push_str(&fmted);
            }
        }
        if this_is_self {
            self.stk_use_counter += add_size;
        }
        return struct_txt;
    }
}
