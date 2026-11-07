//! 生成するアセンブリコードのオペランドを生成する関数群
//! ## 例
//! ```text
//! movl $1, %eax
//! ```
//! の`$1`や`%eax`を生成

use super::*;

impl AsmEmitter {
    pub(in crate::compiler::asm_gen) fn extract_operand_text(
        &mut self,
        parent_id: usize,
        this_is_self: SelfPtrInfo,
    ) -> Result<String, err::ErrKind> {
        match self.inst_at(parent_id) {
            inst::Inst::Num { value, .. } => Ok(self.asm_fmt.get_fmt_num(&value)),
            inst::Inst::Float { .. } => {
                let label = self
                    .static_label(parent_id)
                    .expect("float literal data label not found");
                Ok(self.asm_fmt.fmt_static_var_rip(&label))
            }
            inst::Inst::GetPtr { size: _, stk } => {
                // スタック上に置かれた値そのもの(値が置かれているメモリ)
                // を指すオペランドを、`%rbp`からのオフセットを使って生成する
                // (構造体の実体は`alloc_struct_stk`で確保したオフセットを使う)
                let stk = self.ptr_stk(parent_id, stk);
                Ok(self.rbp_ref(stk))
            }
            inst::Inst::Param(param) => {
                // `asm_emitter/operand_txt/`に記述
                self.param_ref(&param.name)
            }
            inst::Inst::AssignVar { ref name, .. } => self.var_operand(name),
            // 配列にアクセス
            inst::Inst::InsertArr { name, dst, index } => {
                // `asm_emitter/operand_txt/`に記述
                self.insert_arr_txt(&name, dst, index, this_is_self)
            }
            inst::Inst::Str { .. } => {
                // `asm_emitter/operand_txt/`に記述
                self.string_mem_ref(parent_id)
            }
            // 文字列の長さの静的データ(`M0Len(%rip)`)
            inst::Inst::StrLen { .. } => {
                let label = self.static_label(parent_id).ok_or_else(|| {
                    err::ErrKind::asm_emit_label_not_found("string len", parent_id)
                })?;
                Ok(self.asm_fmt.fmt_static_var_rip(&label))
            }
            inst::Inst::Mov { ref name, src, .. } => {
                // `asm_emitter/operand_txt/`に記述
                self.gen_mov_code(name.as_deref(), src)
            }
            inst::Inst::Block(name) => Ok(name.to_string()),
            inst::Inst::ExpectJmp(name) => Ok(name.to_string()),
            inst::Inst::Struct { mem, .. } => {
                self.emit_struct_init_asm(
                    mem,
                    // Noneの場合それはSelf
                    this_is_self.is_none(),
                )
            }
            inst::Inst::Variant {
                tag,
                value,
                size,
                is_self,
                ..
            } => self.emit_data_variant_init_asm(parent_id, value.as_ref(), tag, is_self, &size),
            inst::Inst::MemoryValue(inst::MemoryInst::Memory { kind, size, .. }) => {
                // `asm_emitter/operand_txt/`に記述
                self.ref_mem_value_txt(&kind, &size, parent_id)
            }
            inst::Inst::RefStruct { src, pos, size } => {
                // `asm_emitter/operand_txt/`に記述
                self.ref_struct_txt(&src, pos, size.to_bytes())
            }
            // `self->name`: 構造体を指すポインタ変数`src`の先のメンバー。
            // ポインタ変数は(レジスタまたは退避先から読み込んだ)ポインタの値を
            // ベースにするので、構造体の先頭を指すポインタのメンバー参照と同じ形になる
            inst::Inst::RefStructPtr { src, pos, size } => {
                self.ref_struct_txt(&src, pos, size.to_bytes())
            }
            inst::Inst::GetAddress(index) => self.extract_operand_text(index, this_is_self),
            // 配列リテラル自体を値として参照する場合
            // (例: 変数に束縛されずそのまま関数の引数などに使われる`{1,2,3}`)
            inst::Inst::InitArr(ids) => {
                // `asm_emitter/operand_txt/`に記述
                self.init_arr_txt::<false>(&ids, this_is_self)?;
                Ok(String::new())
            }
            inst::Inst::CallFunc(call_fn_info) => {
                self.call_result_operand(parent_id, &call_fn_info, this_is_self)
            }
            inst::Inst::Pointer(index) => self.extract_operand_text(index, this_is_self),
            t => self.expr_result_operand(parent_id, &t),
        }
    }

    /// 関数呼び出しのノード`parent_id`を値として使う際のオペランドを返す
    ///
    /// まだ呼び出しのアセンブリが生成されていなければここで生成し、
    /// 戻り値のレジスタ(0番、`%eax`など)を返す
    pub(super) fn call_result_operand(
        &mut self,
        parent_id: usize,
        call_fn_info: &inst::CallFuncMetaData,
        this_is_self: SelfPtrInfo,
    ) -> Result<String, err::ErrKind> {
        // 関数呼び出しのアセンブリが`build_fn_process`側で
        // 既に生成済みか
        let already_emitted = self.fn_state.emitted_calls.contains(&parent_id);

        if !already_emitted && call_fn_info.parent != crate::ir::IS_ASSIGN_EXPR {
            // 呼び出しを生成する対象ではない
            return Ok(String::new());
        }
        if !already_emitted {
            let call_asm = self.emit_call_func(
                call_fn_info,
                matches!(this_is_self, Some(types::Size::Struct(..))),
            )?;
            self.asm_text.push_str(&call_asm);
        }
        // 戻り値のレジスタ(0番、`%eax`など)を返す。
        // 戻り値を受ける側の型(`this_is_self`)がある
        // 場合はそのサイズ、なければ64bit(`%rax`)
        let ret_size = this_is_self.map(Self::value_reg_size).unwrap_or(Size::DQ);
        if ret_size.is_float() {
            self.asm_fmt.get_fmt_float_reg(0, &ret_size)
        } else {
            self.asm_fmt.get_fmt_reg(0, &ret_size)
        }
    }

    /// 式などの計算結果を持つレジスタのオペランドを返す
    /// (`last_inst_idx`に結果のレジスタが記録されているノード)
    pub(super) fn expr_result_operand(
        &mut self,
        parent_id: usize,
        node: &inst::Inst,
    ) -> Result<String, err::ErrKind> {
        if let Some((_, offset)) = self
            .fn_state
            .spilled_expr_results
            .iter()
            .find(|(id, _)| *id == parent_id)
        {
            return Ok(self.rbp_ref(*offset));
        }
        if let Some(result) = self
            .fn_state
            .last_inst_idx
            .iter()
            .find(|i| i.0 == parent_id)
        {
            // 式の結果を持つレジスタは、その式自身の型のサイズで
            // 取得する(常に64bitにすると、`movl %rcx, %edx`の
            // ように32bitの命令へ64bitのレジスタが混ざる)
            let reg = result.1;
            let size = match node {
                inst::Inst::Expr(..) => Self::value_reg_size(&self.get_expr_ty(parent_id)),
                // キャストの結果は、キャスト先の型のサイズで参照する
                inst::Inst::Cast { after, .. } => Self::value_reg_size(after),
                _ => Size::DQ,
            };
            match reg {
                RegisterMode::Float(index) => self.asm_fmt.get_fmt_float_reg(index, &size),
                RegisterMode::Normal(index) => self.asm_fmt.get_fmt_reg(index, &size),
            }
        } else {
            panic!("{:?}", node);
        }
    }
}
