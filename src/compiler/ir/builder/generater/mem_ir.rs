//! メモリ(スタック/静的領域)の確保と、メモリ上の変数定義のIR生成

use super::*;

impl IR {
    pub(in crate::compiler::ir) fn stack_counter(&mut self, size: &ast::TyNode) {
        let result = self.try_size_or_emit_struct(size);
        let ty = self.unwrap_or_report(result);
        self.stk_counter += ty.to_bytes();
    }

    /// スタック/静的領域に確保する変数の定義から
    /// `inst::Inst::Struct(Vec<inst::MemoryInst>)` を生成する
    ///
    /// - `name: [ty] = 100` のように長さの指定が無い場合は要素数1
    /// - `name: [ty 4] = {100, 100, 100, 100}` のように配列リテラルが
    ///   与えられた場合は、要素ごとに値を生成する
    pub(in crate::compiler::ir) fn gen_mem_def_var<M: ResultMod>(
        &mut self,
        mut var: ast::DefineVar,
    ) -> Result<inst::Inst, err::ErrKind> {
        let (ref ty_name, len, is_static) = match &var.ty {
            ast::TyNode::Stack { name, len } => (name.clone(), *len, false),
            ast::TyNode::Static { name, len } => (name.clone(), *len, true),
            _ => panic!("gen_mem_def_var: スタック/静的領域の型ではありません"),
        };
        let elem_size = self.size_of(&ty_name);

        // 配列リテラルなら要素ごとに展開し、単一の値ならそのまま1要素として扱う
        let values = match (*var.value).node {
            ast::ExprNode::Array(items) => items,
            _ => vec![(*var.value).clone()],
        };

        let mut value_idx = values
            .iter()
            .map(|v| M::handle(self.gen_expr_ir::<M>(v, Some(&elem_size))))
            .collect::<Result<Vec<_>, _>>()?;

        // メモリ領域の情報を作成
        // 先頭にどの領域(スタック/静的)かを示し、続けて要素分のバイト数を積む
        let mem_kind = if is_static {
            inst::MemoryKind::Static
        } else {
            inst::MemoryKind::Stack
        };
        let mem_insts = inst::MemoryInst::Memory {
            name: var.name.to_string(),
            size: elem_size.clone(),
            src: value_idx,
            kind: mem_kind,
            dst: self.ir_tree.len(),
        };
        for _ in 0..len {
            //mem_insts.push(inst::MemoryInst::Byte(elem_size.to_bytes()));
        }

        // 変数はスタック/静的領域に配置された値を指す必要があるため、
        // 生成した値そのもの(last_value_idx)ではなく、
        // この`MemoryValue`命令自身のid(=mem_insts.dst)を
        // 変数の指す先として登録する。
        // (以前はlen == 1の場合に値のidをそのまま使っていたため、
        //  変数を参照した際に即値やレジスタが直接使われてしまい、
        //  スタック/静的領域への書き込みが無視されるバグがあった)
        let var_idx = self.id_counter;
        let _ = self.var_tree.push::<'l'>(
            &mem::take(&mut var.name),
            var_idx,
            &var.ty,
            &var.var_attr,
            Rc::clone(&self.current_span),
            || Self::wrap_put_var_def(&mut self.contract_flag, &var.ty, &self.current_span),
        )?;

        Ok(inst::Inst::MemoryValue(mem_insts))
    }
}

#[cfg(test)]
mod mem_var_tests;
