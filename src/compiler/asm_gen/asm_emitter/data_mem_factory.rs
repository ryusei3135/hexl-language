//! メモリに配置するデータを生成するモジュール

use super::*;

impl AsmEmitter {
    /// 構造体のメモリをスタック領域に配置するコードを生成
    pub(in crate::compiler::asm_gen) fn emit_struct_init_asm(
        &mut self,
        struct_node: Vec<inst::MemoryInst>,
        this_is_self: bool,
    ) -> Result<String, err::ErrKind> {
        let mut struct_txt = String::new();
        let mut add_size = 0;
        for member in struct_node.clone().iter() {
            let inst::MemoryInst::Member {
                value_idx, size, ..
            } = member
            else {
                return Err(err::ErrKind::asm_emit_not_member_inst(format!(
                    "{:?}",
                    member
                )));
            };

            let member_size = Some(size);

            // メンバーが構造体を返す関数の呼び出し(`orig: Cmdln::new()`)の場合、
            // 呼び出し先が書き込む領域(隠しの`self`ポインタが指す先)を、
            // 別のスタック領域ではなく、外側の構造体の中のこのメンバーの位置にする。
            // 呼び出しは`extract_operand_text`で生成されるので、その前に位置を決める
            if let Some((arg_idx, ptr_idx)) = self.nested_struct_ctor_ptr(*value_idx, this_is_self)
            {
                add_size += size.to_bytes();
                let offset = self.make_member_offset(this_is_self, add_size, size)?;
                if this_is_self {
                    // `self`ポインタ(`offset`は`self`からのオフセット)を元に、
                    // 引数を作る時点でアドレスを計算する
                    self.fn_state.nested_self_arg.insert(arg_idx, offset);
                } else {
                    self.fn_state.struct_stk_map.insert(ptr_idx, offset);
                }
                // ここまでのメンバーの書き込みを、呼び出しより先に出力する
                // (呼び出しでレジスタが壊れ、先のメンバーの値が失われないように)
                self.asm_text.push_str(&struct_txt);
                struct_txt.clear();
                // 呼び出しのアセンブリは`asm_text`へ積まれる。戻り値のオペランドは空
                let _ = self.extract_operand_text(*value_idx, member_size)?;
                continue;
            }

            let value = self
                .extract_operand_text(*value_idx, member_size)?
                .to_string();
            if value.is_empty() {
                let inst::Inst::InitArr(arr) = self.inst_at(*value_idx) else {
                    return Err(err::ErrKind::asm_emit_not_init_arr(*value_idx));
                };
                struct_txt.push_str(self.init_arr_txt::<true>(&arr, member_size)?.as_str());
                continue;
            }
            // このメンバー分を足した「累積」サイズ
            // (これが、このメンバーの`%rbp`からのオフセットになる)
            add_size += size.to_bytes();

            // ポインタは先頭のメンバーを指す(先頭のメンバーのオフセットは0)ので、
            // このメンバーより前のメンバーの累積サイズがオフセットになる
            let offset = self.make_member_offset(this_is_self, add_size, &size)?;

            let fmted = self.asm_fmt.get_fmt_struct_member(value, &size, offset);

            if this_is_self {
                // 第一引数(`self`のポインタ)のレジスタを取得し、
                // `%rbp`をそのレジスタに置き換える
                // (ポインタなので64bitのレジスタ(`Size::DQ`)を使う)
                // 退避している場合は、書き込みの直前に退避先からポインタを読み込む
                let (load, base) = self.self_ptr_base()?;
                struct_txt.push_str(&load);
                struct_txt.push_str(&fmted.replace(FRAME_BASE_REG, &base));
            } else {
                struct_txt.push_str(&fmted);
            }
        }
        if this_is_self {
            self.fn_state.stk_use_counter += add_size;
        }

        Ok(struct_txt)
    }

    /// `value_idx`が構造体を返す関数の呼び出しなら、
    /// `(隠しの`self`引数のノードid, その参照先の`GetPtr`のノードid)`を返す
    ///
    /// `this_is_self`のときは、引数を`self`ポインタから計算するので、
    /// 引数が隠しの`self`だけの呼び出し(`Cmdln::new()`など)に限る
    fn nested_struct_ctor_ptr(&self, value_idx: usize, this_is_self: bool) -> Option<(usize, usize)> {
        let inst::Inst::CallFunc(meta_data) = self.inst_at(value_idx) else {
            return None;
        };
        if !matches!(&meta_data.ret_ty, Some(types::Size::Struct(..))) {
            return None;
        }
        if this_is_self && meta_data.params.len() != 1 {
            return None;
        }
        let self_arg = *meta_data.params.get(0)?;
        let inst::Inst::GetAddress(mem_idx) = self.inst_at(self_arg) else {
            return None;
        };
        matches!(self.inst_at(mem_idx), inst::Inst::GetPtr { .. }).then_some((self_arg, mem_idx))
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
    pub(in crate::compiler::asm_gen) fn emit_data_variant_init_asm(
        &mut self,
        parent_id: usize,
        member: Option<&inst::MemoryInst>,
        tag: usize,
        this_is_self: bool,
        variant_size: &Size,
    ) -> Result<String, err::ErrKind> {
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
            } = mem_val
            else {
                return Err(err::ErrKind::asm_emit_not_member_inst(format!(
                    "{:?}",
                    mem_val
                )));
            };

            let member_size = Some(size);
            let value = self
                .extract_operand_text(*value_idx, member_size)?
                .to_string();
            if value.is_empty() {
                let inst::Inst::InitArr(arr) = self.inst_at(*value_idx) else {
                    return Err(err::ErrKind::asm_emit_not_init_arr(*value_idx));
                };
                variant_txt.push_str(self.init_arr_txt::<true>(&arr, member_size)?.as_str());
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
            let (load, base) = self.self_ptr_base()?;
            variant_txt = format!("{}{}", load, variant_txt.replace(FRAME_BASE_REG, &base));
        }
        Ok(variant_txt)
    }

    fn emit_variant_tag_field(&mut self, offset: usize, tag_num: usize) -> String {
        self.asm_fmt.get_fmt_struct_member(
            self.asm_fmt.get_fmt_num(&tag_num.to_string()),
            &types::Size::DD,
            offset,
        )
    }

    /// 構造体やバリアントのoffsetを計算
    fn make_member_offset(
        &self,
        this_is_self: bool,
        add_size: usize,
        size: &Size,
    ) -> Result<usize, err::ErrKind> {
        // ポインタは先頭のメンバーを指す(先頭のメンバーのオフセットは0)ので、
        // このメンバーより前のメンバーの累積サイズがオフセットになる
        let member_off = add_size.checked_sub(size.to_bytes()).ok_or_else(|| {
            err::ErrKind::asm_emit_member_offset_underflow(add_size, size.to_bytes())
        })?;
        Ok(if this_is_self {
            // `self`のポインタ先に直接書き込むので、既存の
            // `stk_use_counter`(このスコープでのスタック使用量)は
            // 無関係
            member_off
        } else {
            // 既に使用していたスタックのサイズ + 前のメンバーの累積サイズ
            self.fn_state.stk_use_counter + member_off
        })
    }
}
