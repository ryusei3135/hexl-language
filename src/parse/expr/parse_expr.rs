use crate::token::token;
use crate::parse::node;
use crate::parse::resp;
use crate::parse::expr::parse_trim::parse_trim;


pub fn parse_expr(tokens: Vec<token::Token>, index: &mut i32) -> node::CalculNode {
    let mut node = parse_trim(tokens.clone(), index);

    while tokens.len() as i32 > *index {
        match tokens[*index as usize].kind {
            token::TokenKind::TokenAdd => {
                *index += 1;
                let right = parse_trim(tokens.clone(), index);
                node = resp::handler::make_operator_node(
                    node,
                    right,
                    node::NodeKind::NodeAdd,
                );
            }
            token::TokenKind::TokenSub => {
                let operator = &tokens[*index as usize];
                *index += 1;
                let right = parse_trim(tokens.clone(), index);
                node = resp::handler::make_operator_node(
                    node,
                    right,
                    node::NodeKind::NodeSub,
                );
            }
            token::TokenKind::TokenSpace => {
                *index += 1;
            }
            _ => {
                break;
            }
        }
    }

    return node;
}
