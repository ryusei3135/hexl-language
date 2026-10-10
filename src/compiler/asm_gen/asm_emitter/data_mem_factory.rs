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

    /// バリアント(`E::a`など)をスタック領域に配置するコードを生成
    ///
    /// ポインタは先頭のフィールド(タグ、4byte)を指し、ペイロード
    /// (最大バリアントのサイズ`P`)はその下に並ぶ。
    /// ```text
    ///   tag     : 0(ptr)
    ///   payload : -P(ptr)      ([ptr - P, ptr)に収まり、タグと重ならない)
    /// ```
    /// `this_is_self`が`false`の場合、`ptr`は`%rbp - N`で、`N`は
    /// `alloc_struct_stk(parent_id, P)`で確保する(`stk_use_counter`を進める)。
    /// `true`の場合は`self`のポインタ先へ直接書き込む。
    ///
    /// 返すのは命令列のみ。呼び出し側(`format_line`)が、この後に
    /// アドレスをレジスタへ渡す命令(`lea`)を続ける。
    pub(in crate::compiler::asm_gen) 
    fn emit_data_variant_init_asm(
        &mut self, 
        parent_id: usize,
        member: Option<&inst::MemoryInst>, 
        tag: usize, 
        this_is_self: bool, 
        variant_size: &Size
    ) -> String {
        let payload_bytes = variant_size.extract_variant_largest_size().to_bytes();
        let (tag_off, data_off) = if this_is_self {
            (0, payload_bytes)
        } else {
            let n = self.alloc_struct_stk(parent_id, payload_bytes);
            (n, n + payload_bytes)
        };

        let mut variant_txt = String::new();
        // ペイロード(中身がないバリアントは書き込まない)
        if let Some(mem_val) = member {
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
            } else {
                variant_txt.push_str(&self.asm_fmt.get_fmt_struct_member(
                    value,
                    variant_size.extract_variant_largest_size(),
                    data_off,
                ));
            }
        }
        variant_txt.push_str(self.emit_variant_tag_field(tag_off, tag).as_str());

        if this_is_self {
            // `self`のポインタ(64bitのレジスタ)の指す先へ書き込む
            variant_txt = variant_txt.replace("%rbp", &self.self_ptr_reg());
        }
        variant_txt
    }

    fn emit_variant_tag_field(&mut self, offset: usize, tag_num: usize) -> String {
        self.asm_fmt.get_fmt_struct_member(self.asm_fmt.get_fmt_num(&tag_num.to_string()), &types::Size::DD, offset)
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