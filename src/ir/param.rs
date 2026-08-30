use super::*;

impl IR {
    /// 関数のノードを生成するときに、引数を登録
    #[inline(always)]
    pub fn push_param_meta_data(
        &mut self, 
        params: &Vec<node::ArgsNode>
    ) {
        for (index, param) in params.iter().enumerate() {
            self.var_tree.push::<'p'>(&param.name, &index, &param.ty);
            self.ir_tree
                .push(inst::Inst::Param(inst::ParamMetaData::new(
                    param.name.to_string(),
                    index,
                    self.ir_tree.len(),
                    &param.ty,
                )));
            self.id_counter += 1;
        }
    }
}
