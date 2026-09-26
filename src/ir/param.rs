use super::*;

impl IR {
    /// 関数のノードを生成するときに、引数を登録
    #[inline(always)]
    pub fn push_param_meta_data(&mut self, params: &[node::ArgsNode]) -> Result<(), err::ErrKind> {
        for (index, param) in params.iter().enumerate() {
            let _ = self.var_tree.push::<'p'>(
                &param.name,
                index,
                &param.ty,
                &param.var_attr,
                || self.constract_flag.put_var_def(&param.ty),
            )?;
            // 型がポインタの先に構造体がある場合、その構造体のサイズを取得する
            // types::Struct(struct_ty)
            let size = if let Some(s) = types::Size::new(&param.ty) {
                s
            } else {
                // get_struct_size で発生したエラーを一時的に保存する変数
                let mut err_stored = None;

                let emitted_size = types::Size::emit_struct_ty_node(
                    &mut |name| {
                        self.struct_tree.get_struct_size(name)
                    },
                    &param.ty
                );
                if let Some(err) = err_stored {
                    return Err(err);
                }
                emitted_size?
            };

            self.ir_tree
                .push(inst::Inst::Param(inst::ParamMetaData::new(
                    param.name.to_string(),
                    index,
                    self.ir_tree.len(),
                    &size,
                )));
            self.id_counter += 1;
        }
        Ok(())
    }
}
