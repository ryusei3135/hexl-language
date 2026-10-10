//! メモリに配置するデータを生成するモジュール

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
            let member_off = add_size - size.to_bytes();
            struct_txt.push_str(&self.fmt_member_write(value, size, member_off, this_is_self));
        }
        if this_is_self {
            self.stk_use_counter += add_size;
        }
        return struct_txt;
    }

    /// メンバー1つ分の書き込みコードを生成する
    ///
    /// `member_off`はポインタ(先頭のメンバー)からのオフセット。
    /// `this_is_self`なら`self`のポインタ先へ直接、そうでなければ
    /// 既に使用していたスタックのサイズ分ずらして`%rbp`基準で書き込む
    fn fmt_member_write(
        &self,
        value: String,
        size: &Size,
        member_off: usize,
        this_is_self: bool,
    ) -> String {
        if this_is_self {
            // `self`のポインタ先に直接書き込むので、既存の
            // `stk_use_counter`(このスコープでのスタック使用量)は無関係
            let fmted = self.asm_fmt.get_fmt_struct_member(value, size, member_off);
            // 第一引数(`self`のポインタ)のレジスタを取得し、
            // `%rbp`をそのレジスタに置き換える
            // (ポインタなので64bitのレジスタ(`Size::DQ`)を使う)
            fmted.replace("%rbp", &self.self_ptr_reg())
        } else {
            self.asm_fmt
                .get_fmt_struct_member(value, size, self.stk_use_counter + member_off)
        }
    }

    /// バリアント型のメモリをスタック領域に配置するコードを生成
    ///
    /// - データ付きメンバー(`value`が`Some`)は構造体として扱い、
    ///   `[タグ(DD)][ペイロード]`(`tagged`でなければ`[ペイロード]`)の
    ///   メンバーを並べて書き込む
    /// - データなしメンバー(`value`が`None`)は整数型として扱い、
    ///   タグの値を`DD`の整数としてそのまま書き込む
    ///   (`tagged`でない型なしメンバーは書き込む値がないので何も出力しない)
    pub(in crate::compiler::asm_gen)
    fn emit_data_variant_init_asm(
        &mut self,
        tag: usize,
        value: Option<inst::MemoryInst>,
        tagged: bool,
        size: &Size,
        this_is_self: bool,
    ) -> String {
        let mut txt = String::new();
        let mut off = 0;

        if tagged {
            let tag_size = Size::DD;
            let tag_txt = self.asm_fmt.get_fmt_num(&tag.to_string());
            txt.push_str(&self.fmt_member_write(tag_txt, &tag_size, off, this_is_self));
            off += types::VARIANT_TAG_BYTES;
        }

        // データなし: 整数型(タグのみ)として展開済み
        if let Some(member) = value {
            // データあり: 構造体として、タグの後ろにペイロードを置く
            let inst::MemoryInst::Member {
                value_idx,
                size: member_size,
                ..
            } = member
            else {
                panic!("バリアント型のメンバーは`MemoryInst::Member`である必要があります");
            };
            let operand = self
                .extract_operand_text(value_idx, Some(&member_size))
                .to_string();
            if operand.is_empty() {
                // 配列リテラルなどはオペランドを持たず、直接メモリへ展開する
                let inst::Inst::InitArr(arr) = self.inst_at(value_idx) else {
                    panic!();
                };
                txt.push_str(self.init_arr_txt::<true>(&arr, Some(&member_size)).as_str());
            } else {
                txt.push_str(&self.fmt_member_write(operand, &member_size, off, this_is_self));
            }
        }

        if this_is_self {
            self.stk_use_counter += size.to_bytes();
        }
        txt
    }
}