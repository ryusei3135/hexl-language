use crate::token::token;
use crate::parse::node;
use crate::parse::expr::parse_comper;
use crate::parse::resp;


pub fn make_ret_node(tokens: Vec<token::Token>, index: &mut i32) -> node::CalculNode {
    let mut ret_node = resp::handler::make_null_node();
    let mut ret_token: bool = false;
    ret_node.node_type = node::NodeKind::NodeRet;

    while tokens.len() > *index as usize {
        match tokens[*index as usize].kind {
            token::TokenKind::TokenRet => ret_token = true,
            token::TokenKind::TokenSpace => {}
            _ => {
                if ret_token {
                    ret_node.left_node = Some(
                        Box::new(
                            parse_comper::parse_comper_op(
                                tokens.clone(),
                                index
                            )
                        )
                    );
                }
            }
        }
        *index += 1;
    }

    ret_node
}
