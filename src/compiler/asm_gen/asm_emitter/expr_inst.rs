//! 式(`Inst::Expr`)のテンプレートを組み立てる処理

use super::*;

impl AsmEmitter {
    pub(in crate::compiler::asm_gen) fn format_expr_inst(
        &mut self,
        expr: &inst::ExprInst,
        size: Option<Size>,
    ) -> Result<String, err::ErrKind> {
        // `/`と`%`は`%rax`/`%rdx`を使う専用の手順で生成する
        // (`div_ir.rs`)。テンプレートでは、割られる数を`%rax`ではなく
        // `%rdx`に置いてしまい、さらに`%rdx`にある引数を壊していた
        if matches!(expr.kind, inst::ExprKind::Div | inst::ExprKind::Surplus) {
            return self.format_div_expr_inst(expr);
        }

        let key = Self::expr_tmpl_key(&expr, &self.fn_state.curr_inst)?;
        let mnemonic = Self::expr_mnemonic(&expr.kind)?;

        let resolved_size: Size = if let Some(t) = size {
            t
        } else {
            self.get_expr_ty(expr.ls)
        };
        // 定数との乗算(`x * 2`/`x * 8`/`x * 3`など)は、`imul`ではなく
        // `add`・シフト・`lea`にする(`mul_opt.rs`)
        if expr.kind == inst::ExprKind::Mul {
            if self.optimization_mode != OptMode::O0 {
                if let Some(optimized) = self.format_const_mul(expr, &resolved_size)? {
                    return Ok(optimized);
                }
            }
        }
        if matches!(expr.kind, inst::ExprKind::Inc | inst::ExprKind::Dec) {
            let is_memory_access = self.check_node_is_mem_val(expr.ls).is_some();
            let operand = self.extract_operand_text(expr.ls, None)?;
            let dst = self.get_reg_txt(
                Some(self.fn_state.reg_idx),
                &Self::value_reg_size(&resolved_size),
            )?;
            let mut output = self.mov_line(&dst, &operand, &resolved_size, is_memory_access);
            let mut update = self.asm_fmt.get_opcode_tmpl(key).replace("{dst}", &operand);
            update = if is_memory_access {
                self.asm_fmt
                    .fmt_memory_mnemonic_resize(mnemonic, &update, &resolved_size)
            } else {
                self.asm_fmt
                    .fmt_mnemonic_resize(mnemonic, &update, &resolved_size)
            };
            output.push_str(&update);
            return Ok(output);
        }
        // `<`/`>`は、どちらかのオペランドが符号なしなら符号なしの比較
        // (`jl`/`jg`ではなく`jb`/`ja`)にする。`==`/`!=`は符号に関係ない
        let is_unsigned_cmp = matches!(
            expr.kind,
            inst::ExprKind::LessThen
                | inst::ExprKind::GreaterThen
                | inst::ExprKind::LessThenEq
                | inst::ExprKind::GreaterThenEq
        ) && (resolved_size.is_unsigned()
            || self.get_expr_ty(expr.ls).is_unsigned()
            || self.get_expr_ty(expr.rs).is_unsigned());
        let wrap_size = resolved_size.wrap_dst_size();

        let dst_text = self.get_reg_txt(Some(self.fn_state.reg_idx), &resolved_size)?;
        let src1_text = self.extract_operand_text(expr.ls, wrap_size.as_ref())?;

        let mut tmpl = if is_unsigned_cmp {
            self.asm_fmt.get_unsigned_cmp_tmpl(key)
        } else {
            self.asm_fmt.get_opcode_tmpl(key)
        };
        if expr.kind == inst::ExprKind::Mul {
            tmpl = tmpl.replace("add", "imul");
        }

        let src2_text = self.extract_operand_text(expr.rs, wrap_size.as_ref())?;
        let both_memory = self.check_node_is_mem_val(expr.ls).is_some()
            && self.check_node_is_mem_val(expr.rs).is_some();
        let mut prefix = String::new();
        let src1_text = if both_memory {
            prefix.push_str(&self.mov_line(&dst_text, &src1_text, &resolved_size, true));
            dst_text.clone()
        } else {
            src1_text
        };
        let mut formated = tmpl
            .replace("{dst}", &dst_text)
            .replace("{src1}", &src1_text)
            .replace("{src2}", &src2_text)
            .to_string();
        if !prefix.is_empty() {
            formated = format!("{prefix}{formated}");
        }

        let is_memory_access = self.check_node_is_mem_val(expr.ls).is_some()
            || self.check_node_is_mem_val(expr.rs).is_some();
        formated = self.fmt_one_expr_mnemo_resize(
            formated.to_string(),
            &resolved_size,
            mnemonic,
            is_memory_access,
        );
        if dst_text == src1_text
            && matches!(
                expr.kind,
                inst::ExprKind::Add
                    | inst::ExprKind::Sub
                    | inst::ExprKind::AndCalcul
                    | inst::ExprKind::XorCalcul
                    | inst::ExprKind::OrCalcul
                    | inst::ExprKind::LShift
                    | inst::ExprKind::RShift
                    | inst::ExprKind::Mul
                    | inst::ExprKind::LessThen
                    | inst::ExprKind::GreaterThen
                    | inst::ExprKind::LessThenEq
                    | inst::ExprKind::GreaterThenEq
                    | inst::ExprKind::Equal
                    | inst::ExprKind::NotEq
            )
        {
            let self_move = self.mov_line(&dst_text, &src1_text, &resolved_size, false);
            formated = formated.replacen(&self_move, "", 1);
        }

        // サイズがSelfでない場合
        if self.fn_state.reserved_label_name.is_some() && formated.contains("{label}") {
            let name = self.fn_state.reserved_label_name.take().unwrap();
            Ok(formated.replace("{label}", &name))
        } else {
            Ok(formated)
        }
    }
}
