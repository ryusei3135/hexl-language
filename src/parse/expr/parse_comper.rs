use crate::token::token;
use crate::parse::node;
use crate::parse::expr::parse_expr;
use crate::parse::resp;



pub fn parse_comper_op(tokens: Vec<token::Token>, index: &mut i32) -> node::CalculNode {
    let mut node = parse_expr::parse_expr(tokens.clone(), index);

    while tokens.len() > *index as usize {
        match tokens[*index as usize].kind {
            token::TokenKind::TokenEqTo => {
                *index += 1;
                let right = parse_expr::parse_expr(tokens.clone(), index);
                node = resp::handler::make_operator_node(
                    node,
                    right,
                    node::NodeKind::NodeEqTo,
                );
            },
            token::TokenKind::TokenNotEqTo => {
                *index += 1;
                let right = parse_expr::parse_expr(tokens.clone(), index);
                node = resp::handler::make_operator_node(
                    node,
                    right,
                    node::NodeKind::NodeNotEqTo,
                );
            },
            token::TokenKind::TokenSpace => {
                *index += 1;
            },
            _ => {
                break;
            }
        }
    }

    node
}
