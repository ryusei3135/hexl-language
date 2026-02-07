use crate::token::token;
use crate::parse::node;
use crate::parse::resp;
use crate::parse::expr::parse_trim::parse_trim;


pub fn parse_expr(tokens: Vec<token::Token>, index: &mut usize) -> node::CalculNode {
    let mut node = parse_trim(tokens.clone(), index);

    while tokens.len() > *index {
        match tokens[*index].kind {
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
