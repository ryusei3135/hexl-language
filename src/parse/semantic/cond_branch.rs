use crate::token::token;
use crate::parse::node;
use crate::parse::resp;
use crate::parse::expr::parse_comper::parse_comper_op;


pub fn make_if_node(tokens: Vec<token::Token>, index: &mut i32) -> node::CalculNode {
    let mut if_node = resp::handler::make_null_node();

    while tokens.len() > *index as usize {
        match tokens[*index as usize].kind {
            token::TokenKind::TokenSpace => {},
            token::TokenKind::TokenIf => if_node.node_type = node::NodeKind::NodeIf,
            token::TokenKind::TokenLBrace => {
                break;
            }
            _ => {
                if_node.left_node = Some(Box::new(parse_comper_op(tokens.clone(), index)));
                continue;
            }
        }
        *index += 1;
    }

    if_node
}
