use super::*;


pub fn make_ret_node(tokens: Vec<token::Token>, index: &mut usize) -> Result<node::CalculNode, err_kind::ErrorsKind> {
    let mut ret_node = resp::handler::make_null_node();
    let mut ret_token: bool = false;
    ret_node.node_type = node::NodeKind::NodeRet;

    while tokens.len() > *index {
        match tokens[*index].kind {
            token::TokenKind::TokenRet => ret_token = true,
            token::TokenKind::TokenSpace => {}
            _ => {
                if ret_token {
                    ret_node.left_node = Some(
                        Box::new(
                            parse_comper::parse_comper_op(
                                tokens.clone(),
                                index
                            )?
                        )
                    );
                }
            }
        }
        *index += 1;
    }

    Ok(ret_node)
}
