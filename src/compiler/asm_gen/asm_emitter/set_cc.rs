//! 比較の結果(0/1)を値として得る命令(`Set<比較>`)のアセンブリの生成
//!
//! `ExprKind::SetLessThen`などは、`ls`と`rs`を比較して、結果の0/1を
//! レジスタへ置く。次の3命令になる。
//!
//! ```text
//!   cmp   rs, ls          フラグを設定する(`tcmp`)
//!   setcc dst8            条件が真なら1、偽なら0を8bitのレジスタへ書く(`set_*`)
//!   movzbl dst8, dst32    32bitへゼロ拡張する(`movzbl`。上位32bitも0になる)
//! ```
//!
//! - 結果は`ls`の型のレジスタ(`get_expr_ty`)として読まれる。8bit・16bit・32bitの値は
//!   `dst32`の下位、64bitの値は`movzbl`が上位32bitも0にするので`dst64`としてそのまま読める。
//! - `<` `>` `<=` `>=`は、どちらかのオペランドが符号なしなら符号なしの比較
//!   (`setb`/`seta`/`setbe`/`setae`。`get_unsigned_set_tmpl`)。`==`/`!=`は符号に関係ない。
//! - `cmp`の比較される側(`ls`)は即値にできないので、即値のときは結果のレジスタへ
//!   一時的に`mov`して使う(`dst`は`ls`/`rs`のレジスタとは別に確保する)。
//! - `setcc`は`cmp`のフラグを読むだけなので、`cmp`と`setcc`の間に他の命令を挟まない。
//!   `movzbl`はフラグを変えないが、`setcc`のあとに置く。
//!
//! ## 制限
//! - 小数(`F32`/`F64`)は使えない(`comiss`/`comisd`のフラグはNaNの扱いが整数と違う)

use super::*;

impl AsmEmitter {
    /// `Set<比較>`の種類か
    pub(in crate::compiler::asm_gen) fn is_set_cc_kind(kind: &inst::ExprKind) -> bool {
        matches!(
            kind,
            inst::ExprKind::SetLessThen
                | inst::ExprKind::SetGreaterThen
                | inst::ExprKind::SetLessThenEq
                | inst::ExprKind::SetGreaterThenEq
                | inst::ExprKind::SetEqual
                | inst::ExprKind::SetNotEq
        )
    }

    /// 種類から、`setcc`のテンプレートを引くキーを返す
    fn set_cc_key(kind: &inst::ExprKind) -> &'static str {
        match kind {
            inst::ExprKind::SetLessThen => "set_l",
            inst::ExprKind::SetGreaterThen => "set_g",
            inst::ExprKind::SetLessThenEq => "set_l_e",
            inst::ExprKind::SetGreaterThenEq => "set_g_e",
            inst::ExprKind::SetEqual => "set_e",
            inst::ExprKind::SetNotEq => "set_ne",
            other => panic!("setccの式ではありません: {:?}", other),
        }
    }

    /// `Set<比較>`: `cmp` → `setcc` → `movzbl`で、結果の0/1をレジスタへ置く
    pub(in crate::compiler::asm_gen) fn gen_set_cc_asm(&mut self, expr: &inst::ExprInst) {
        let ls_ty = self.get_expr_ty(expr.ls);
        if ls_ty.is_float() || self.get_expr_ty(expr.rs).is_float() {
            panic!("setccの比較に小数は使えません");
        }
        let size = Self::value_reg_size(&ls_ty);
        let wrap_size = size.wrap_dst_size();

        // 結果を置くレジスタ。`ls`/`rs`のレジスタとは別のものになる
        // (それらは、この命令が終わるまで解放しない)
        let dst = self.alloc_reg(&Size::DD);
        self.reg_idx = dst.index();
        self.used_reg.mark_used(&dst);
        let dst8 = self.get_reg(Some(dst.index()), &Size::DB);
        let dst32 = self.get_reg(Some(dst.index()), &Size::DD);

        let mut src1 = self.extract_operand_text(expr.ls, wrap_size.as_ref());
        let src2 = self.extract_operand_text(expr.rs, wrap_size.as_ref());
        let is_memory_access = self.check_node_is_mem_val(expr.ls).is_some()
            || self.check_node_is_mem_val(expr.rs).is_some();

        let mut output = String::new();
        // `cmp`の比較される側(`src1`)は即値にできないので、結果のレジスタへ移す
        // (`setcc`で上書きするので、そのレジスタは一時的に使える)
        if matches!(self.inst_at(expr.ls), inst::Inst::Num { .. }) {
            let reg_txt = self.get_reg(Some(dst.index()), &size);
            output.push_str(&self.mov_line(&reg_txt, &src1, &size, false));
            src1 = reg_txt;
        }

        let cmp = self
            .asm_fmt
            .get_opcode_tmpl("tcmp")
            .replace("{src1}", &src1)
            .replace("{src2}", &src2);
        output.push_str(&self.fmt_one_expr_mnemo_resize(cmp, &size, "cmp", is_memory_access));

        // `<` `>` `<=` `>=`は、どちらかのオペランドが符号なしなら符号なしの比較
        let is_unsigned_cmp = matches!(
            expr.kind,
            inst::ExprKind::SetLessThen
                | inst::ExprKind::SetGreaterThen
                | inst::ExprKind::SetLessThenEq
                | inst::ExprKind::SetGreaterThenEq
        ) && (ls_ty.is_unsigned() || self.get_expr_ty(expr.rs).is_unsigned());
        let key = Self::set_cc_key(&expr.kind);
        let set_tmpl = if is_unsigned_cmp {
            self.asm_fmt.get_unsigned_set_tmpl(key)
        } else {
            self.asm_fmt.get_opcode_tmpl(key)
        };
        // `setcc`/`movzbl`にサイズ接尾辞は付けない(オペランドは8bit/32bitのレジスタで決まる)
        output.push_str(&set_tmpl.replace("{dst}", &dst8));
        output.push_str(
            &self
                .asm_fmt
                .get_opcode_tmpl("movzbl")
                .replace("{src1}", &dst8)
                .replace("{dst}", &dst32),
        );
        self.asm_text.push_str(&output);

        // 式の結果を置いたレジスタを使用中として記録する
        self.last_inst_idx.push((expr.dst, dst));
        // 比較し終わったので、オペランドの一時的な結果のレジスタを解放する
        self.release_expr_temp(expr.ls);
        self.release_expr_temp(expr.rs);
    }
}
