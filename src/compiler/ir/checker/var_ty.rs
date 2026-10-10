
use super::*;

impl IR {
    /// 一回しか呼ばれないのでinline
    /// 代入先の型と代入する型が同じか比較する。
    #[inline(always)]
    pub(in crate::compiler::ir) 
    fn check_var_ty(&self, var_ty: &types::Size, expect_ty: &types::Size) {
        // let ty = self.var_tree.get_ty_node(var_name).unwrap();
        println!("{:?} fond var_ty {:?}", expect_ty, var_ty);

        match expect_ty {
            types::Size::Pointer { ty, is_const, range } => {
                if let types::Size::GetAddr(var_t) = var_ty {
                    if var_t != ty {
                        panic!();
                    }
                }
            }
            _ => {
                if var_ty != expect_ty {
                    // let changed_ty: types::Size = ty.clone().try_into().unwrap();
                    panic!("{:?} fond {:?}", expect_ty, var_ty);
                }
            }
        }
    }
}
