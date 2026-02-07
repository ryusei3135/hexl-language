use super::*;


pub fn make_for_node(
        tokens: Vec<token::Token>,
        index: &mut usize
) -> node::CalculNode {
    *index += 1;
    let mut for_loop_node = resp::handler::make_null_node();
    let mut first_token = (token::TokenKind::TokenEOF, String::new());

    while tokens.len() > *index {
        match tokens[*index].kind {
            token::TokenKind::TokenNum => {
                first_token = (token::TokenKind::TokenNum, tokens[*index].lexeme.clone());
                for_loop_node = expr::parse_expr::parse_expr(tokens.clone(), index);
            }
            token::TokenKind::TokenLBrace => {
                break;
            }
            token::TokenKind::TokenName => {
                if first_token.0 == token::TokenKind::TokenEOF {
                    first_token = (token::TokenKind::TokenName, tokens[*index].lexeme.clone());
                } else {
                    println!("syntax err for semantic");
                    std::process::exit(1);
                }
            }
            token::TokenKind::TokenIn => {
                if first_token.0 == token::TokenKind::TokenName {
                    *index += 1;
                    for_loop_node = node::CalculNode {
                        value: first_token.1.clone(),
                        node_type: node::NodeKind::NodeIn,
                        left_node: Some(Box::new(expr::parse_expr::parse_expr(tokens.clone(), index))),
                        right_node: None,
                        block: Some(tokens[0].line),
                    };
                }
            }
            token::TokenKind::TokenSpace => {}
            _ => {}
        }
        *index += 1;
    }

    node::CalculNode {
        value: String::new(),
        node_type: node::NodeKind::NodeFor,
        left_node: Some(Box::new(for_loop_node)),
        right_node: None,
        block: Some(tokens[0].line),
    }
}
