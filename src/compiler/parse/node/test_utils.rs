use super::*;


#[cfg(test)]
pub fn gen_var_node(name: &str, value: &str, ty: &str, line: usize) -> Group2Info {
    Group2Node::Expr(Expr::DefVar(DefineVar::new(
        &name.to_string(),
        Expr::Number(value.to_string()),
        &TyNode::Ty(ty.to_string()),
        VarMutAttr::Invar,
    )))
    .gen_group_info(line)
}

#[cfg(test)]
pub fn wrap_expr_cmp(left: &str, right: &str) -> Expr {
    Expr::LessThen((
        Box::new(Expr::Number(left.to_string())),
        Box::new(Expr::Number(right.to_string())),
    ))
}

#[cfg(test)]
pub fn wrap_eq_expr_cmp(left: &str, right: &str) -> Expr {
    Expr::Equal((
        Box::new(Expr::Number(left.to_string())),
        Box::new(Expr::Number(right.to_string())),
    ))
}
