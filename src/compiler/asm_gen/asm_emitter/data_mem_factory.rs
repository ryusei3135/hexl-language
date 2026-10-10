//! メモリに配置するデータを生成するモジュール

use crate::compiler::ir::inst::MemoryInst::Member;

use super::*;

impl AsmEmitter {
    /// 構造体のメモリをスタック領域に配置するコードを生成
    pub(in crate::compiler::asm_gen) 
    fn emit_struct_init_asm(
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

            let member_size = Some(size);

            let value = self
                .extract_operand_text(*value_idx, member_size)
                .to_string();
            if value.is_empty() {
                let inst::Inst::InitArr(arr) = self.inst_at(*value_idx) else {
                    panic!();
                };
                struct_txt.push_str(self.init_arr_txt::<true>(&arr, member_size).as_str());
                continue;
            }
            // このメンバー分を足した「累積」サイズ
            // (これが、このメンバーの`%rbp`からのオフセットになる)
            add_size += size.to_bytes();

            // ポインタは先頭のメンバーを指す(先頭のメンバーのオフセットは0)ので、
            // このメンバーより前のメンバーの累積サイズがオフセットになる
            let offset = self.make_member_offset(this_is_self, add_size, &size);

            let fmted = self.asm_fmt.get_fmt_struct_member(value, &size, offset);

            if this_is_self {
                // 第一引数(`self`のポインタ)のレジスタを取得し、
                // `%rbp`をそのレジスタに置き換える
                // (ポインタなので64bitのレジスタ(`Size::DQ`)を使う)
                struct_txt.push_str(&fmted.replace("%rbp", &self.self_ptr_reg()));
            } else {
                struct_txt.push_str(&fmted);
            }
        }
        if this_is_self {
            self.stk_use_counter += add_size;
        }
        
        struct_txt
    }

    pub(in crate::compiler::asm_gen)
    fn emit_data_variant_init_asm(&mut self, name: String, member: Option<&inst::MemoryInst>, tag: usize, this_is_self: bool, variant_size: &Size) -> String {
        let mut variant_txt = String::new();
        let mut offset = 0;
        let txt = if let Some(mem_val) = member {
            let inst::MemoryInst::Member {
                value_idx, size, ..
            } = mem_val else {
                panic!("{:?}", member);
            };

            let member_size = Some(size);
            let value = self
                .extract_operand_text(*value_idx, member_size)
                .to_string();
            if value.is_empty() {
                let inst::Inst::InitArr(arr) = self.inst_at(*value_idx) else {
                    panic!();
                };
                variant_txt.push_str(self.init_arr_txt::<true>(&arr, member_size).as_str());
            }
            
            // このメンバー分を足した「累積」サイズ
            // (これが、このメンバーの`%rbp`からのオフセットになる)
            let add_size = size.to_bytes();

            // ポインタは先頭のメンバーを指す(先頭のメンバーのオフセットは0)ので、
            // このメンバーより前のメンバーの累積サイズがオフセットになる
            offset = self.make_member_offset(this_is_self, add_size, &size);

            self.asm_fmt.get_fmt_struct_member(value, &variant_size, offset)
        } else {
            offset = self.make_member_offset(this_is_self, variant_size.to_bytes(), &variant_size);
            self.asm_fmt.get_fmt_struct_member("0".to_string(), &variant_size, offset)
        };

        variant_txt.push_str(txt.as_str());
        variant_txt.push_str(self.emit_variant_tag_field(offset, tag).as_str());

        variant_txt
    }

    fn emit_variant_tag_field(&mut self, offset: usize, tag_num: usize) -> String {
        self.asm_fmt.get_fmt_struct_member(tag_num.to_string(), &types::Size::DD, offset)
    }

    /// 構造体やバリアントのoffsetを計算
    fn make_member_offset(&self, this_is_self: bool, add_size: usize, size: &Size) -> usize {
        // ポインタは先頭のメンバーを指す(先頭のメンバーのオフセットは0)ので、
        // このメンバーより前のメンバーの累積サイズがオフセットになる
        let member_off = add_size - size.to_bytes();
        if this_is_self {
            // `self`のポインタ先に直接書き込むので、既存の
            // `stk_use_counter`(このスコープでのスタック使用量)は
            // 無関係
            member_off
        } else {
            // 既に使用していたスタックのサイズ + 前のメンバーの累積サイズ
            self.stk_use_counter + member_off
        }
    }
}
