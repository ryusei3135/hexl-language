use super::*;


pub fn make_for_node(
        tokens: Vec<token::Token>,
        index: &mut i32
) -> node::CalculNode {
    *index += 1;
    let mut for_loop_node = resp::handler::make_null_node();

    while tokens.len() > *index as usize {
        match tokens[*index as usize].kind {
            token::TokenKind::TokenNum => {
                for_loop_node = expr::parse_expr::parse_expr(tokens.clone(), index);
            }
            token::TokenKind::TokenLBrace => {
                break;
            }
            // token::TokenKind::TokenNewVar => {
            //     //  この時点でtoken(TokenIn)が来ることが確定する
            //     expr::parse_def::parse_var_def(tokens, index);
            // }
            // token::TokenKind::TokenIn => {}
            token::TokenKind::TokenSpace => {}
            _ => {}
        }
        *index += 1;
    }

    return node::CalculNode {
        value: "[*for*]".to_string(),
        node_type: node::NodeKind::NodeFor,
        left_node: Some(Box::new(for_loop_node)),
        right_node: None,
        block: None
    };
}
