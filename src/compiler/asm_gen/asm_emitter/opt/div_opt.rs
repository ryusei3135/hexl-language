//! 定数(2のべき乗)での除算(`x / 2^k`)を、`idiv`/`div`を使わずシフトで生成するAPI
//!
//! `format_div_expr_inst`(`div_ir.rs`)の先頭からのみ呼び出す。`%`は対象外。
//!
//! ## 生成する命令(`dst`は結果のレジスタ、`src`は割られる数)
//! 符号なし(`2^k`で割るのは論理右シフトと同じ):
//! ```text
//! mov src, dst
//! shr $k, dst
//! ```
//! 符号付き(`sar`だけだと負数が0方向ではなく負の無限大方向へ丸まるので、
//! 負数のときだけ`2^k - 1`を足してからシフトする):
//! ```text
//! mov src, dst
//! (使用中の%raxがあれば push)
//! mov dst, %rax        ; 以下、%raxはサイズに合わせた名前(%eax/%rax)
//! sar $(bits-1), %rax  ; 負数なら全ビット1、そうでなければ0
//! shr $(bits-k), %rax  ; 負数なら 2^k - 1、そうでなければ0
//! add %rax, dst
//! (push した%raxを pop)
//! sar $k, dst
//! ```
//! - 符号付きは32/64bitだけを対象にする(8/16bitは従来の`idiv`)。
//! - 符号なしは、型自身の幅(8/16/32/64bit)のままシフトする。
//! - 除数が`1`・2のべき乗でない・型の幅以上・負数・割られる数も定数の場合や、
//!   浮動小数点数の場合は`Ok(None)`を返し、従来の`idiv`/`div`に任せる。

use super::*;
use crate::compiler::asm_gen::sign::SizeSign;

impl AsmEmitter {
    /// 除数が2のべき乗の整数リテラルなら、シフト量`k`を返す。
    /// 割る数と割られる数は交換できないので、見るのは右辺だけ。割られる数も定数なら対象外
    fn const_div_shift(&self, expr: &inst::ExprInst, bits: u32) -> Option<u32> {
        let as_const = |idx: usize| -> Option<i128> {
            match self.fn_state.curr_inst.get(idx)? {
                inst::Inst::Num { value, .. } => value.replace('_', "").parse::<i128>().ok(),
                _ => None,
            }
        };
        as_const(expr.ls).map_or(Some(()), |_| None)?;
        let c = as_const(expr.rs)?;
        if c < 2 || c & (c - 1) != 0 {
            return None;
        }
        let k = c.trailing_zeros();
        (k < bits).then_some(k)
    }

    /// `オペコードのテンプレート(key)`の`{dst}`と`{src1}`を埋め、サイズ接尾辞を調整した1行
    fn div_opt_line(&self, key: &str, mnemonic: &str, dst: &str, src: &str, size: &Size) -> String {
        let line = self
            .asm_fmt
            .tmpl_or_default(key)
            .replace("{dst}", dst)
            .replace("{src1}", src);
        self.asm_fmt.fmt_mnemonic_resize(mnemonic, &line, size)
    }

    /// `x / 2^k`のアセンブリを返す。対象でない場合は`Ok(None)`
    /// (呼び出し側が従来の`idiv`/`div`を生成する)
    pub fn format_const_div(
        &mut self,
        expr: &inst::ExprInst,
    ) -> Result<Option<String>, err::ErrKind> {
        if expr.kind != inst::ExprKind::Div || expr.is_float(&self.fn_state.curr_inst) {
            return Ok(None);
        }
        let ty = self.get_expr_ty(expr.ls);
        // `div_ir.rs`と同じ。どちらかが符号なしなら符号なしの除算
        let unsigned = ty.is_unsigned() || self.get_expr_ty(expr.rs).is_unsigned();
        let size = Self::value_reg_size(&ty);
        let bits = match size {
            Size::DB => 8,
            Size::DW => 16,
            Size::DD => 32,
            Size::DQ => 64,
            _ => return Ok(None),
        };
        // 符号付きの8/16bitは従来の`idiv`
        if !unsigned && bits < 32 {
            return Ok(None);
        }
        let Some(k) = self.const_div_shift(expr, bits) else {
            return Ok(None);
        };

        let wrap_size = ty.wrap_dst_size();
        let src = self.extract_operand_text(expr.ls, wrap_size.as_ref())?;
        let is_mem = self.check_node_is_mem_val(expr.ls).is_some();
        let dst = self.get_reg_txt(Some(self.fn_state.reg_idx), &size)?;
        let count = |n: u32| self.asm_fmt.get_fmt_num(&n.to_string());

        // `src`と`dst`が同じレジスタのとき、`mov r, r`は不要
        let mut asm = if src == dst {
            String::new()
        } else {
            self.mov_line(&dst, &src, &size, is_mem)
        };

        if unsigned {
            asm.push_str(&self.div_opt_line("div_shr", "shr", &dst, &count(k), &size));
            return Ok(Some(asm));
        }

        // 符号付き: 負数のときだけ`2^k - 1`を足してから`sar`する。
        // 補正値の計算に`%rax`を使うので、使用中なら退避する
        let acc = self.asm_fmt.get_fmt_reg(0, &size)?;
        let saved = self.live_reserved_regs(self.fn_state.reg_idx).contains(&0);
        let acc64 = self.asm_fmt.get_fmt_reg(0, &Size::DQ)?;
        if saved {
            asm.push_str(&self.asm_fmt.get_push(&acc64));
        }
        asm.push_str(&self.mov_line(&acc, &dst, &size, false));
        asm.push_str(&self.div_opt_line("div_sar", "sar", &acc, &count(bits - 1), &size));
        asm.push_str(&self.div_opt_line("div_shr", "shr", &acc, &count(bits - k), &size));
        asm.push_str(&self.div_opt_line("div_add", "add", &dst, &acc, &size));
        if saved {
            asm.push_str(&self.asm_fmt.get_pop(&acc64));
        }
        asm.push_str(&self.div_opt_line("div_sar", "sar", &dst, &count(k), &size));
        Ok(Some(asm))
    }
}
