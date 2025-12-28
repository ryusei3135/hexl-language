use crate::token::token;
use crate::parse::node;
use crate::parse::resp;
use crate::parse::expr::parse_expr::parse_expr;


pub fn parse_factor(tokens: Vec<token::Token>, index: &mut i32) -> node::CalculNode {
    let current_token = &tokens[*index as usize];

    match current_token.kind {
        token::TokenKind::TokenNum => {
            *index += 1;
            return resp::handler::make_value_node(
                current_token,
                node::NodeKind::NodeNum,
            );
        }
        token::TokenKind::TokenName => {
            *index += 1;
            return resp::handler::make_value_node(
                current_token,
                node::NodeKind::NodeCallVar,
            );
        }
        token::TokenKind::TokenLParen => {
            *index += 1; // '('をスキップ
            let node = parse_expr(tokens.clone(), index);
            if tokens[*index as usize].kind == token::TokenKind::TokenRParen {
                *index += 1; // ')'をスキップ
            } else {
                println!("Error: Expected ')'");
            }
            return node;
        }
        token::TokenKind::TokenSpace => {
            *index += 1;
            return parse_factor(tokens, index);
        }
        _ => {
            println!("Error: Unexpected token {:?}", current_token);
            return node::CalculNode {
                value: String::new(),
                node_type: node::NodeKind::NodeNull,
                left_node: None,
                right_node: None,
            };
        }
    }
}
