use crate::parse::node;
use crate::token::token;
use crate::parse::expr::parse_factor::parse_factor;
use crate::parse::resp;


pub fn parse_trim(tokens: Vec<token::Token>, index: &mut usize) -> node::CalculNode {
    let mut node = parse_factor(tokens.clone(), index);

    while tokens.len() > *index {
        match tokens[*index].kind {
            token::TokenKind::TokenMul => {
                *index += 1;
                let right = parse_factor(tokens.clone(), index);
                node = resp::handler::make_operator_node(
                    node,
                    right,
                    node::NodeKind::NodeMul,
                );
            }
            token::TokenKind::TokenDiv => {
                *index += 1;
                let right = parse_factor(tokens.clone(), index);
                node = resp::handler::make_operator_node(
                    node,
                    right,
                    node::NodeKind::NodeDiv,
                );
            }
            token::TokenKind::TokenSpace => {
                *index += 1;
            }
            _ => break,
        }
    }

    return node;
}
