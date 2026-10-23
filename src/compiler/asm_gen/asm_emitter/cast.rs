//! `inst::Inst::Cast { src, after }`(組み込み型へのキャスト)のアセンブリを生成する
//!
//! 結果は、新しく確保したレジスタに置き、`last_inst_idx`へ記録する
//! (式`Inst::Expr`と同じ扱い)。結果のレジスタは、`after`のサイズの名前で参照される
//! (`expr_result_operand`)。
//!
//! | 変換 | 生成する命令 |
//! |------|--------------|
//! | 整数 → 大きい整数 | 符号付きは符号拡張(`movsbl`/`movslq`など)、符号なしはゼロ拡張(`movzbl`/`movl`など) |
//! | 整数 → 同じ幅/小さい整数 | 元の幅で`mov`し、結果は小さいサイズのレジスタ名で参照する(`%ecx` → `%cl`) |
//! | 整数 → `float`/`f64` | `cvtsi2ss`/`cvtsi2sd`(`l`は32bit、`q`は64bitの整数) |
//! | `float`/`f64` → 整数(小数点以下は切り捨て) | `cvttss2si`/`cvttsd2si` |
//! | `float` ⇔ `f64` | `cvtss2sd`/`cvtsd2ss`(同じ型なら`movss`/`movsd`) |
//!
//! ## 制限
//! - `cvtsi2ss`/`cvtsi2sd`は符号付きの整数を変換する。そのため`u64`(`UDQ`)は
//!   符号付きの`i64`として変換する(2^63以上の値は負の数になる)。
//!   `float`/`f64` → `u64`も同様に符号付きの`cvttss2si`で変換する。
//! - 8/16bitの整数は、先に32bitへ拡張してから変換する。`uint`(`UDD`)は64bitへ
//!   ゼロ拡張してから変換する(`cvtsi2ssq`)。
//! - `cvt*`のテンプレートは`asm_fmt/x64.rs`にある(AT&T構文)。
//!   Intel構文のフォーマットでは、同じキーを追加する必要がある。

use super::*;

/// 整数のバイト数に対応する、拡張命令のサフィックス(`b`/`w`/`l`/`q`)
fn width_suffix(bytes: usize) -> char {
    match bytes {
        1 => 'b',
        2 => 'w',
        4 => 'l',
        _ => 'q',
    }
}

impl AsmEmitter {
    /// `Inst::Cast { src, after }`のアセンブリを生成する
    ///
    /// ## 引数
    /// - node_idx: `Cast`ノード自身のid(結果のレジスタを`last_inst_idx`に記録する)
    /// - src: キャストされる値のノードのid
    /// - after: キャスト先の型(組み込み型)
    pub(in crate::compiler::asm_gen) fn gen_cast_asm(
        &mut self,
        node_idx: usize,
        src: usize,
        after: &Size,
    ) -> Result<(), err::ErrKind> {
        let before = self.get_expr_ty(src);
        // 変換元の値のオペランド(レジスタ/メモリ/即値)
        let operand = self.extract_operand_text(src, None)?;
        let is_imm = matches!(self.inst_at(src), inst::Inst::Num { .. });
        let is_mem = self.check_node_is_mem_val(src).is_some();

        // 結果を置くレジスタ。変換の途中で使う一時レジスタと被らないよう、先に使用中にする
        let dst = self.alloc_reg(after);
        self.used_reg.mark_used(&dst);
        self.reg_idx = dst.index();

        let asm = match (before.is_float(), after.is_float()) {
            (false, false) => {
                self.cast_int_to_int(&operand, &before, after, dst, is_imm, is_mem)?
            }
            (false, true) => self.cast_int_to_float(&operand, &before, after, dst, is_imm)?,
            (true, false) => self.cast_float_to_int(&operand, &before, after, dst)?,
            (true, true) => self.cast_float_to_float(&operand, &before, after, dst)?,
        };
        self.asm_text.push_str(&asm);

        self.last_inst_idx.push((node_idx, dst));
        // キャスト元が式の一時的な結果なら、使い終わったので解放する
        self.release_expr_temp(src);
        Ok(())
    }

    /// 整数 → 整数
    fn cast_int_to_int(
        &mut self,
        operand: &str,
        before: &Size,
        after: &Size,
        dst: RegisterMode,
        is_imm: bool,
        is_mem: bool,
    ) -> Result<String, err::ErrKind> {
        let before_bytes = before.to_bytes();
        let after_bytes = after.to_bytes();
        let after_reg = Self::value_reg_size(after);
        let before_reg = Self::value_reg_size(before);

        // 即値は、キャスト先の幅でそのまま置く
        if is_imm {
            let dst_txt = self.asm_fmt.get_fmt_reg(dst.index(), &after_reg)?;
            return Ok(self.mov_line(&dst_txt, operand, &after_reg, false));
        }

        // 小さくする(同じ幅も含む): 元の幅で読み込み、結果は小さい幅の名前で参照する
        if after_bytes <= before_bytes {
            let dst_txt = self.asm_fmt.get_fmt_reg(dst.index(), &before_reg)?;
            return Ok(self.mov_line(&dst_txt, operand, &before_reg, is_mem));
        }

        // 大きくする
        let dst_txt = self.asm_fmt.get_fmt_reg(dst.index(), &after_reg)?;
        // 符号なしの32bit → 64bitは、32bitレジスタへの`mov`で上位32bitが0になる
        if before.is_unsigned() && before_bytes == 4 {
            let dst32 = self.asm_fmt.get_fmt_reg(dst.index(), &Size::DD)?;
            return Ok(self.mov_line(&dst32, operand, &Size::DD, is_mem));
        }
        Ok(self.asm_fmt.get_int_extend(
            !before.is_unsigned(),
            width_suffix(before_bytes),
            width_suffix(after_bytes),
            operand,
            &dst_txt,
        ))
    }

    /// 整数 → 浮動小数点数(`cvtsi2ss`/`cvtsi2sd`)
    fn cast_int_to_float(
        &mut self,
        operand: &str,
        before: &Size,
        after: &Size,
        dst: RegisterMode,
        is_imm: bool,
    ) -> Result<String, err::ErrKind> {
        let bytes = before.to_bytes();
        let unsigned = before.is_unsigned();
        // `cvtsi2ss`は32bit/64bitの整数だけを受け付け、即値も受け付けない。
        // 8/16bitの整数・即値・符号なしの32bitは、一時レジスタへ読み込んでから変換する
        let need_tmp = is_imm || bytes < 4 || (bytes == 4 && unsigned);
        // 64bitの整数として変換するか(`uint`はゼロ拡張した64bitで変換する)
        let use_q = bytes == 8 || (bytes == 4 && unsigned);

        let mut asm = String::new();
        let src_txt = if need_tmp {
            let tmp = self.alloc_reg(&Size::DQ);
            let tmp32 = self.asm_fmt.get_fmt_reg(tmp.index(), &Size::DD)?;
            if is_imm {
                // 即値: 32bitまでは`movl`、64bitは`movq`で置く
                let size = if bytes == 8 { Size::DQ } else { Size::DD };
                let tmp_txt = self.asm_fmt.get_fmt_reg(tmp.index(), &size)?;
                asm.push_str(&self.mov_line(&tmp_txt, operand, &size, false));
            } else if bytes < 4 {
                // 8/16bit: 32bitへ拡張する
                asm.push_str(&self.asm_fmt.get_int_extend(
                    !unsigned,
                    width_suffix(bytes),
                    'l',
                    operand,
                    &tmp32,
                ));
            } else {
                // 符号なしの32bit: `movl`で上位32bitが0になる
                asm.push_str(&self.mov_line(&tmp32, operand, &Size::DD, false));
            }
            let size = if use_q { Size::DQ } else { Size::DD };
            self.asm_fmt.get_fmt_reg(tmp.index(), &size)?
        } else {
            operand.to_string()
        };

        let key = match (after, use_q) {
            (Size::F32, false) => "cvtsi2ssl",
            (Size::F32, true) => "cvtsi2ssq",
            (_, false) => "cvtsi2sdl",
            (_, true) => "cvtsi2sdq",
        };
        let dst_txt = self.asm_fmt.get_fmt_float_reg(dst.index(), after)?;
        asm.push_str(&self.fill_tmpl(key, &dst_txt, &src_txt));
        Ok(asm)
    }

    /// 浮動小数点数 → 整数(`cvttss2si`/`cvttsd2si`。小数点以下は切り捨て)
    fn cast_float_to_int(
        &mut self,
        operand: &str,
        before: &Size,
        after: &Size,
        dst: RegisterMode,
    ) -> Result<String, err::ErrKind> {
        let key = if matches!(before, Size::F32) {
            "cvttss2si"
        } else {
            "cvttsd2si"
        };
        // 結果は32bit/64bitのレジスタに書き込まれる。8/16bitの結果は、その下位を参照する。
        // `uint`は64bitで変換して、下位32bitを使う(2^31以上の値を正しく変換するため)
        let after_bytes = after.to_bytes();
        let wide = after_bytes == 8 || (after_bytes == 4 && after.is_unsigned());
        let conv_size = if wide { Size::DQ } else { Size::DD };
        let dst_txt = self.asm_fmt.get_fmt_reg(dst.index(), &conv_size)?;
        Ok(self.fill_tmpl(key, &dst_txt, operand))
    }

    /// 浮動小数点数 → 浮動小数点数(`float` ⇔ `f64`)
    fn cast_float_to_float(
        &mut self,
        operand: &str,
        before: &Size,
        after: &Size,
        dst: RegisterMode,
    ) -> Result<String, err::ErrKind> {
        let key = match (before, after) {
            (Size::F32, Size::F64) => "cvtss2sd",
            (Size::F64, Size::F32) => "cvtsd2ss",
            (Size::F32, _) => "float_mov",
            _ => "f64_mov",
        };
        let dst_txt = self.asm_fmt.get_fmt_float_reg(dst.index(), after)?;
        Ok(self.fill_tmpl(key, &dst_txt, operand))
    }
}
