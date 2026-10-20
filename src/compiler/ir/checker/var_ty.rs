use super::*;

impl IR {
    /// 一回しか呼ばれないのでinline
    /// 代入先の型と代入する型が同じか比較する。
    #[inline(always)]
    pub(in crate::compiler::ir) fn check_var_ty(
        &self,
        var_ty: &types::Size,
        expect_ty: &types::Size,
    ) {
        // let ty = self.var_tree.get_ty_node(var_name).unwrap();

        match expect_ty {
            types::Size::Pointer { ty, .. } => {
                if let types::Size::GetAddr(var_t) = var_ty {
                    if var_t != ty {
                        panic!();
                    }
                }
            }
            // 期待される型が配列のとき、indexが`DQ`(i64)ならそのまま通す
            types::Size::Array { .. } if matches!(var_ty, types::Size::DQ) => {}
            _ => {
                // 配列の添字アクセス(`[arr i else 0]`)の基底として配列変数を使うとき、
                // 期待される型は配列そのものではなく要素の型になる
                // (`ref_array_node`/`insert_guarded_store`が配列の変数を`Var`として生成する)
                if let types::Size::Array { size, .. } = var_ty {
                    let elem: &types::Size = size;
                    if elem == expect_ty {
                        return;
                    }
                } else {
                    if var_ty != expect_ty {
                        // let changed_ty: types::Size = ty.clone().try_into().unwrap();
                        panic!("{:?} fond {:?}", expect_ty, var_ty);
                    }
                }
            }
        }
    }
}