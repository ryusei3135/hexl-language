//! 定数との乗算(`x * C`)を、`imul`を使わない命令で生成するAPI
//!
//! `format_expr_inst`からのみ呼び出す。置き換えの方針(どの定数をどうするか)は
//! `optimization/x64/asm_gen/mul_strength.rs`の`plan_mul`が決める。
//!
//! ## 生成する命令(`dst`は結果のレジスタ、`src`は定数でない側のオペランド)
//! ```text
//! x * 2      mov src, dst ; add dst, dst
//! x * 2^k    mov src, dst ; shl $k, dst
//! x * 3/5/9  lea (src64,src64,2/4/8), dst
//! x * 7      lea 0(,src64,8), dst ; sub src, dst
//! ```
//! - `src`がメモリ(`-4(%rbp)`など)の場合、`lea`のアドレスには使えないので、
//!   先に`mov src, dst`でレジスタへ読み込み、その`dst`(64bit)を使う。
//! - `lea`にはサイズ接尾辞が付かない8/16bitの形が無いので、`dst`が64bit以外
//!   (8/16/32bit)のときは32bitの`leal`で書く。下位ビットは乗算の結果と同じ。
//!   (`lea`に`l`/`q`を自分で付けるのは、`ensure_mnemonic_size`が`lea`を
//!   常に`q`にしてしまうため。接尾辞が付いた命令はそのまま残る)
//! - 乗算は符号の有無に関係なく下位ビットが同じなので、符号付き・なしで共通。
//! - `x * 7`で`dst`と`src`が同じレジスタのとき(`dst`を先に書くと`x`が消える)は、
//!   置き換えず通常の乗算に任せる(`Ok(None)`)。

use super::*;
use crate::compiler::optimization::x64::asm_gen::mul_strength::{MulPlan, plan_mul};

impl AsmEmitter {
    /// `expr`(`*`)の片方が整数の定数なら、`(もう片方のid, 定数)`を返す。
    /// 乗算は交換できるので、定数が左辺でも右辺でもよい。両方が定数の場合は
    /// 最適化しない(定数畳み込みの対象)
    fn const_mul_operand(&self, expr: &inst::ExprInst) -> Option<(usize, i128)> {
        let as_const = |idx: usize| -> Option<i128> {
            match self.fn_state.curr_inst.get(idx)? {
                inst::Inst::Num { value, .. } => value.replace('_', "").parse::<i128>().ok(),
                _ => None,
            }
        };
        match (as_const(expr.ls), as_const(expr.rs)) {
            (None, Some(c)) => Some((expr.ls, c)),
            (Some(c), None) => Some((expr.rs, c)),
            _ => None,
        }
    }

    /// 整数の結果の型の幅(ビット)。整数以外(浮動小数点数・ポインタなど)は`None`
    fn int_bits(size: &Size) -> Option<u32> {
        match size {
            Size::DB | Size::UDB => Some(8),
            Size::DW | Size::UDW => Some(16),
            Size::DD | Size::UDD => Some(32),
            Size::DQ | Size::UDQ => Some(64),
            _ => None,
        }
    }

    /// 定数との乗算を最適化したアセンブリを返す。
    /// 最適化の対象でない場合は`Ok(None)`(呼び出し側が通常の乗算を生成する)
    pub fn format_const_mul(
        &mut self,
        expr: &inst::ExprInst,
        size: &Size,
    ) -> Result<Option<String>, err::ErrKind> {
        if expr.is_float(&self.fn_state.curr_inst) {
            return Ok(None);
        }
        let Some(bits) = Self::int_bits(size) else {
            return Ok(None);
        };
        let Some((operand_id, constant)) = self.const_mul_operand(expr) else {
            return Ok(None);
        };
        let Some(plan) = plan_mul(constant, bits) else {
            return Ok(None);
        };

        let wrap_size = size.wrap_dst_size();
        let dst = self.get_reg_txt(Some(self.fn_state.reg_idx), size)?;
        let src = self.extract_operand_text(operand_id, wrap_size.as_ref())?;
        let is_mem = self.check_node_is_mem_val(operand_id).is_some();

        let mut out = match plan {
            MulPlan::SelfAdd => {
                // mov src, dst ; add dst, dst
                let line = self
                    .asm_fmt
                    .tmpl_or_default("add")
                    .replace("{dst}", &dst)
                    .replace("{src1}", &src)
                    .replace("{src2}", &dst);
                self.fmt_one_expr_mnemo_resize(line, size, "add", is_mem)
            }
            MulPlan::Shift(k) => {
                // mov src, dst ; shl $k, dst
                let count = self.asm_fmt.get_fmt_num(&k.to_string());
                let line = self
                    .asm_fmt
                    .tmpl_or_default("lshift")
                    .replace("{dst}", &dst)
                    .replace("{src1}", &src)
                    .replace("{src2}", &count);
                self.fmt_one_expr_mnemo_resize(line, size, "shl", is_mem)
            }
            MulPlan::Lea { scale } => {
                let (load, base) = self.lea_base(&src, &dst, size, is_mem)?;
                let lea = self.lea_line("mul_lea_scale", &base, size, Some(scale))?;
                format!("{}{}", load, lea)
            }
            MulPlan::Lea7 => {
                if !is_mem && src == dst {
                    // `dst`を先に書くと`x`が消えるので、通常の乗算に任せる
                    return Ok(None);
                }
                let (load, base) = self.lea_base(&src, &dst, size, is_mem)?;
                let lea = self.lea_line("mul_lea_x8", &base, size, None)?;
                // 8x - x。`x`はレジスタか、メモリのまま引く
                let sub = self
                    .asm_fmt
                    .tmpl_or_default("sub")
                    .lines()
                    .last()
                    .unwrap_or_default()
                    .replace("{dst}", &dst)
                    .replace("{src2}", &src);
                let sub = format!("{}\n", sub);
                let sub = if is_mem {
                    self.asm_fmt.fmt_memory_mnemonic_resize("sub", &sub, size)
                } else {
                    self.asm_fmt.fmt_mnemonic_resize("sub", &sub, size)
                };
                format!("{}{}{}", load, lea, sub)
            }
        };

        // `src`と`dst`が同じレジスタのとき、`mov r, r`は不要
        if src == dst {
            let self_move = self.mov_line(&dst, &src, size, false);
            out = out.replacen(&self_move, "", 1);
        }
        Ok(Some(out))
    }

    /// `lea`のアドレスのベースにする64bitのレジスタを返す。
    /// `src`がレジスタなら、そのまま64bitにして使う(`load`は空)。
    /// メモリ・即値などの場合は、先に`dst`へ読み込み(`load`)、`dst`の64bitを使う
    fn lea_base(
        &self,
        src: &str,
        dst: &str,
        size: &Size,
        is_mem: bool,
    ) -> Result<(String, String), err::ErrKind> {
        if src.starts_with('%') && !src.contains('(') {
            let base = self.asm_fmt.resize_reg_operand(src, &Size::DQ)?;
            return Ok((String::new(), base));
        }
        let load = self.mov_line(dst, src, size, is_mem);
        let base = self.asm_fmt.resize_reg_operand(dst, &Size::DQ)?;
        Ok((load, base))
    }

    /// `lea`の1行を作る。`dst`のサイズが64bitなら`leaq`、それ以外は`leal`
    /// (結果のレジスタは常に`self.reg_idx`)
    fn lea_line(
        &self,
        key: &str,
        base: &str,
        size: &Size,
        scale: Option<u8>,
    ) -> Result<String, err::ErrKind> {
        let (lea_size, suffix) = if size.to_bytes() == 8 {
            (Size::DQ, "q")
        } else {
            (Size::DD, "l")
        };
        let dst = self.get_reg_txt(Some(self.fn_state.reg_idx), &lea_size)?;
        let mut line = self
            .asm_fmt
            .tmpl_or_default(key)
            .replace("{dst}", &dst)
            .replace("{src1}", base);
        if let Some(scale) = scale {
            line = line.replace("{scale}", &scale.to_string());
        }
        Ok(line.replacen("lea", &format!("lea{}", suffix), 1))
    }
}
