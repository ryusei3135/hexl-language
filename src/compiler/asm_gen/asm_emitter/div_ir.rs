//! `/`(Div)と`%`(Surplus)のアセンブリ言語を生成するAPI
//!
//! `format_expr_inst`からのみ呼び出す。
//!
//! ## なぜテンプレートを使わないのか
//! フォーマットファイルの`div`/`sur`のテンプレートは、割られる数を
//! `%rax`ではなく`%rdx`へ移してから`cltd`していた。
//! `idiv`は`%rdx:%rax`を割られる数として使うので、`%rax`に
//! 何も入っていないまま計算され、結果が壊れる。さらに`%rdx`は
//! 第3引数のレジスタなので、そこに引数があると破壊されてしまう。
//!
//! そのため、ここでは次の手順を直接生成する。
//! ```text
//! (使用中の%rax/%rdxがあれば push)
//! mov <割る数>,   <dst>        ; dstは%rax/%rdx以外の空きレジスタ
//! mov <割られる数>, %rax
//! cqto                         ; 32bitなら cltd
//! idiv <dst>
//! mov %rax(/) か %rdx(%), <dst>
//! (push したものを逆順に pop)
//! ```

use super::*;

impl AsmEmitter {
    /// `/`と`%`のアセンブリ言語を生成する
    ///
    /// 結果は`self.reg_idx`のレジスタに置く。
    /// (`gen_expr_asm`が、`%rax`/`%rdx`以外の空きレジスタを
    /// `self.reg_idx`へ設定してから呼ぶ)
    pub(super) 
    fn format_div_expr_inst(&mut self, expr: &inst::ExprInst) -> String {
        let dst = self.reg_idx;
        let ty = self.get_expr_ty(expr.ls);
        let wrap_size = ty.wrap_dst_size();

        // 64bitの型だけ64bitで計算し、それ以外は32bitで計算する
        // (`idiv`は8/16bitの除算を`%ax`などで行うので、
        //  値を32bitに広げて計算する方が扱いやすい)
        let calc = match Self::value_reg_size(&ty) {
            Size::DQ => Size::DQ,
            _ => Size::DD,
        };

        // オペランドのテキストは、レジスタなら計算サイズに揃える
        let src1 = self.extract_operand_text(expr.ls, &wrap_size);
        let src1 = self.asm_fmt.resize_reg_operand(&src1, &calc);
        let src2 = self.extract_operand_text(expr.rs, &wrap_size);
        let src2 = self.asm_fmt.resize_reg_operand(&src2, &calc);
        let mem1 = self.check_node_is_mem_val(expr.ls).is_some();
        let mem2 = self.check_node_is_mem_val(expr.rs).is_some();

        let rax = self.asm_fmt.get_fmt_reg(0, &calc);
        let dst_txt = self.asm_fmt.get_fmt_reg(dst, &calc);

        let mut asm = String::new();

        // === 書き換えられる`%rax`/`%rdx`のうち、使用中の物を退避 ===
        let saved = self.live_reserved_regs(dst);
        for reg in saved.iter() {
            let name = self.asm_fmt.get_fmt_reg(*reg, &Size::DQ);
            asm.push_str(&self.asm_fmt.get_push(&name));
        }

        // === 割る数を空きレジスタへ ===
        // (`idiv`は即値を直接取れないのと、`%rdx`を書き換える前に
        //  オペランドを読み出しておく必要があるため、先に移す)
        asm.push_str(&self.mov_line(&dst_txt, &src2, &calc, mem2));
        // === 割られる数を`%rax`へ ===
        asm.push_str(&self.mov_line(&rax, &src1, &calc, mem1));

        // === 符号拡張(`%rdx:%rax`)と除算 ===
        let att = self.asm_fmt.mnemonic_size();
        let extend = match (&calc, att) {
            (Size::DQ, true) => "cqto",
            (Size::DQ, false) => "cqo",
            (_, true) => "cltd",
            (_, false) => "cdq",
        };
        asm.push_str(&format!("{{space}}{}\n", extend));
        let idiv = format!("{{space}}idiv {}\n", dst_txt);
        asm.push_str(&self.asm_fmt.fmt_mnemonic_resize("idiv", &idiv, &calc));

        // === 商(`%rax`)か余り(`%rdx`)を、結果のレジスタへ ===
        let result_reg = if expr.kind == inst::ExprKind::Div {
            0
        } else {
            2
        };
        let result = self.asm_fmt.get_fmt_reg(result_reg, &calc);
        asm.push_str(&self.mov_line(&dst_txt, &result, &calc, false));

        // === 退避したレジスタを逆順に復元 ===
        for reg in saved.iter().rev() {
            let name = self.asm_fmt.get_fmt_reg(*reg, &Size::DQ);
            asm.push_str(&self.asm_fmt.get_pop(&name));
        }
        asm
    }
}
