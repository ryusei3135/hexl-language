//! 三項演算子(`cond ? left : right`)のアセンブリの生成
//!
//! IRは、次の並びの命令になる(`ir/builder.rs`の`Expr::Ternary`)。
//!
//! ```text
//!   <left>                          左辺(条件が真のときの値)
//!   <right>                         右辺(条件が偽のときの値)
//!   <cond の準備>                   `&&`/`||`があるときの`AndCalcul`/`OrCalcul`など
//!   Expr { kind: Ternary<比較> }    条件(`ls`と`rs`を比較する)    ← 番号 n - 1
//!   Expr { kind: Ternary }          選ぶ(`ls`=左辺, `rs`=右辺)     ← 番号 n
//! ```
//!
//! - `Ternary<比較>`(`TernaryLessThen`など)は`cmp`だけを出す(フラグを設定する)。
//!   結果のレジスタは作らない(`gen_ternary_cmp_asm`)。
//! - `Ternary`は、直前(番号`n - 1`)の`Ternary<比較>`の種類から`cmov`を決め、
//!   `mov 右辺, dst` → `cmov<cc> 左辺, dst`を出す(`gen_ternary_select_asm`)。
//!
//! 左辺・右辺は条件より先に生成する。`add`や`sub`は`cmp`で設定したフラグを
//! 壊してしまうので、条件(`cmp`)は選ぶ命令の直前に置く。`mov`はフラグを変えない。
//!
//! ## 制限
//! - 小数(`F32`/`F64`)は使えない(`cmov`は整数のレジスタ用)
//! - `cmov`は8bitのレジスタを使えないので、8bitの値は32bitのレジスタ名で操作する

use super::*;

impl AsmEmitter {
    /// 三項演算子の条件(`Ternary<比較>`)の種類か
    pub(in crate::compiler::asm_gen) fn is_ternary_cmp_kind(kind: &inst::ExprKind) -> bool {
        matches!(
            kind,
            inst::ExprKind::TernaryLessThen
                | inst::ExprKind::TernaryGreaterThen
                | inst::ExprKind::TernaryLessThenEq
                | inst::ExprKind::TernaryGreaterThenEq
                | inst::ExprKind::TernaryEqual
                | inst::ExprKind::TernaryNotEq
        )
    }

    /// 条件の種類から、`cmov`のテンプレートを引くキーを返す
    fn ternary_cmov_key(kind: &inst::ExprKind) -> Result<&'static str, err::ErrKind> {
        Ok(match kind {
            inst::ExprKind::TernaryLessThen => "cmov_l",
            inst::ExprKind::TernaryGreaterThen => "cmov_g",
            inst::ExprKind::TernaryLessThenEq => "cmov_l_e",
            inst::ExprKind::TernaryGreaterThenEq => "cmov_g_e",
            inst::ExprKind::TernaryEqual => "cmov_e",
            inst::ExprKind::TernaryNotEq => "cmov_ne",
            other => {
                return Err(err::ErrKind::asm_emit_not_ternary_cond_kind(format!(
                    "{:?}",
                    other
                )));
            }
        })
    }

    /// `cmov`が使えるレジスタのサイズ(8bitは使えないので32bitにする)
    fn cmov_reg_size(size: &Size) -> Size {
        match Self::value_reg_size(size) {
            Size::DB => Size::DD,
            other => other,
        }
    }

    /// `Ternary<比較>`: `cmp`だけを出す(結果のレジスタは作らない)
    pub(in crate::compiler::asm_gen) fn gen_ternary_cmp_asm(
        &mut self,
        expr: &inst::ExprInst,
    ) -> Result<(), err::ErrKind> {
        let ls_ty = self.get_expr_ty(expr.ls);
        if ls_ty.is_float() || self.get_expr_ty(expr.rs).is_float() {
            return Err(err::ErrKind::asm_emit_float_not_supported(
                "三項演算子の条件",
            ));
        }
        let size = Self::value_reg_size(&ls_ty);
        let wrap_size = size.wrap_dst_size();

        let mut src1 = self.extract_operand_text(expr.ls, wrap_size.as_ref())?;
        let src2 = self.extract_operand_text(expr.rs, wrap_size.as_ref())?;
        let is_memory_access = self.check_node_is_mem_val(expr.ls).is_some()
            || self.check_node_is_mem_val(expr.rs).is_some();

        let mut output = String::new();
        // `cmp`の比較される側(`src1`)は即値にできないので、一時レジスタへ移す
        let mut tmp: Option<RegisterMode> = None;
        if matches!(self.inst_at(expr.ls), inst::Inst::Num { .. }) {
            let reg = self.alloc_reg(&size);
            self.used_reg.mark_used(&reg);
            let reg_txt = self.get_reg_txt(Some(reg.index()), &size)?;
            output.push_str(&self.mov_line(&reg_txt, &src1, &size, false));
            src1 = reg_txt;
            tmp = Some(reg);
        }

        let cmp = self
            .asm_fmt
            .get_opcode_tmpl("tcmp")
            .replace("{src1}", &src1)
            .replace("{src2}", &src2);
        output.push_str(&self.fmt_one_expr_mnemo_resize(cmp, &size, "cmp", is_memory_access));
        self.asm_text.push_str(&output);

        if let Some(reg) = tmp {
            self.used_reg.release(&reg);
        }
        // 比較し終わったので、オペランドの一時的な結果のレジスタを解放する
        self.release_expr_temp(expr.ls);
        self.release_expr_temp(expr.rs);
        Ok(())
    }

    /// `Ternary`: 直前の`Ternary<比較>`のフラグで、左辺(真)か右辺(偽)を選ぶ
    pub(in crate::compiler::asm_gen) fn gen_ternary_select_asm(
        &mut self,
        expr: &inst::ExprInst,
    ) -> Result<(), err::ErrKind> {
        // 条件は、選ぶ命令のひとつ前に置かれる(`ir/builder.rs`)
        let cond = match expr.dst.checked_sub(1).and_then(|i| self.curr_inst.get(i)) {
            Some(inst::Inst::Expr(cond)) if Self::is_ternary_cmp_kind(&cond.kind) => cond.clone(),
            other => {
                return Err(err::ErrKind::asm_emit_ternary_cond_not_found(format!(
                    "{:?}",
                    other
                )));
            }
        };

        let value_ty = self.get_expr_ty(expr.ls);
        if value_ty.is_float() || self.get_expr_ty(expr.rs).is_float() {
            return Err(err::ErrKind::asm_emit_float_not_supported("三項演算子の値"));
        }
        // レジスタを確保する前に、`cmov`の種類を確認する
        let key = Self::ternary_cmov_key(&cond.kind)?;
        let size = Self::cmov_reg_size(&value_ty);
        let wrap_size = size.wrap_dst_size();

        // 結果を置くレジスタ。左辺・右辺のレジスタとは別のものになる
        // (それらは、この命令が終わるまで解放しない)
        let dst = self.alloc_reg(&size);
        self.reg_idx = dst.index();
        self.used_reg.mark_used(&dst);
        let dst_txt = self.get_reg_txt(Some(dst.index()), &size)?;

        // 8bitの値のレジスタは、32bitのレジスタ名にそろえる
        let left = self.extract_operand_text(expr.ls, wrap_size.as_ref())?;
        let right = self.extract_operand_text(expr.rs, wrap_size.as_ref())?;
        let left = self.asm_fmt.resize_reg_operand(&left, &size)?;
        let right = self.asm_fmt.resize_reg_operand(&right, &size)?;

        let mut output = String::new();
        // `cmov`の元は即値にできないので、左辺が即値なら一時レジスタへ移す
        // (`mov`はフラグを変えない)
        let mut tmp: Option<RegisterMode> = None;
        let left = if matches!(self.inst_at(expr.ls), inst::Inst::Num { .. }) {
            let reg = self.alloc_reg(&size);
            self.used_reg.mark_used(&reg);
            let reg_txt = self.get_reg_txt(Some(reg.index()), &size)?;
            output.push_str(&self.mov_line(&reg_txt, &left, &size, false));
            tmp = Some(reg);
            reg_txt
        } else {
            left
        };

        // 偽のときの値(右辺)を先に入れておき、真のときだけ左辺で上書きする
        output.push_str(&self.mov_line(&dst_txt, &right, &size, false));

        // `<` `>` `<=` `>=`は、どちらかのオペランドが符号なしなら符号なしの比較(`cmovb`など)
        let is_unsigned_cmp = matches!(
            cond.kind,
            inst::ExprKind::TernaryLessThen
                | inst::ExprKind::TernaryGreaterThen
                | inst::ExprKind::TernaryLessThenEq
                | inst::ExprKind::TernaryGreaterThenEq
        ) && (self.get_expr_ty(cond.ls).is_unsigned()
            || self.get_expr_ty(cond.rs).is_unsigned());
        let tmpl = if is_unsigned_cmp {
            self.asm_fmt.get_unsigned_cmov_tmpl(key)
        } else {
            self.asm_fmt.get_opcode_tmpl(key)
        };
        // `cmov`にサイズ接尾辞は付けない(レジスタの幅で決まる。
        // `fmt_mnemonic_resize`の`mov`の置き換えが`cmov`にも当たってしまうため通さない)
        output.push_str(&tmpl.replace("{src1}", &left).replace("{dst}", &dst_txt));
        self.asm_text.push_str(&output);

        if let Some(reg) = tmp {
            self.used_reg.release(&reg);
        }
        // 式の結果を置いたレジスタを使用中として記録する
        self.last_inst_idx.push((expr.dst, dst));
        self.release_expr_temp(expr.ls);
        self.release_expr_temp(expr.rs);
        Ok(())
    }
}
