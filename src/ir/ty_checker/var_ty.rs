use super::*;


impl IR {
    pub fn check_var_ty(
        &self, 
        var_name: &String,
        expect_ty: &types::Size,
    ) {
        let ty = self
            .var_tree
            .get_ty_node(&var_name)
            .unwrap();

        if types::Size::new(&ty)
            .is_ok_and(|result| !matches!(result, expect_ty))
        {
            panic!("{:?} fond {:?}", expect_ty, types::Size::new(&ty));
        }
    }
}
