use super::*;

impl IR {
    pub fn check_var_ty(&self, var_name: &String, expect_ty: &types::Size) {
        let ty = self.var_tree.get_ty_node(&var_name).unwrap();

        if ty
            .clone()
            .try_into()
            .is_ok_and(|result: types::Size| !matches!(result, expect_ty))
        {
            let ty: types::Size = ty.try_into().unwrap();
            panic!("{:?} fond {:?}", expect_ty, ty);
        }
    }
}
