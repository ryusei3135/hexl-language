#[cfg(test)]
use super::*;
#[cfg(test)]
use crate::compiler::err::Span;
#[cfg(test)]
use std::rc::Rc;

#[cfg(test)]
fn span_at(line: usize) -> Rc<Span> {
    Rc::new(Span::new(line, 0, String::new(), &Rc::new("<test>".to_string())))
}

#[cfg(test)]
pub fn gen_var_node(name: &str, value: &str, ty: &str, line: usize) -> Group2Info {
    Group2Node::Expr(
        DefineVar::new(
            &name.to_string(),
            Expr::new(ExprNode::Number(value.to_string()), span_at(line)),
            &TyNode::Ty(ty.to_string()),
            VarMutAttr::Invar,
        )
        .wrap(span_at(line)),
    )
    .gen_group_info(line, span_at(line))
}

#[cfg(test)]
pub fn wrap_expr_cmp(left: &str, right: &str) -> Expr {
    Expr::new(
        ExprNode::LessThen((
            Box::new(Expr::new(ExprNode::Number(left.to_string()), span_at(0))),
            Box::new(Expr::new(ExprNode::Number(right.to_string()), span_at(0))),
        )),
        span_at(0),
    )
}

#[cfg(test)]
pub fn wrap_eq_expr_cmp(left: &str, right: &str) -> Expr {
    Expr::new(
        ExprNode::Equal((
            Box::new(Expr::new(ExprNode::Number(left.to_string()), span_at(0))),
            Box::new(Expr::new(ExprNode::Number(right.to_string()), span_at(0))),
        )),
        span_at(0),
    )
}
