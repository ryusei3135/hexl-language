use crate::token::token;
use crate::parse::node;
use crate::parse::resp;
use crate::parse::expr::parse_expr::parse_expr;
use crate::parse::expr::parse_comper::parse_comper_op;


fn parse_func_arg(tokens: Vec<token::Token>, index: &mut i32) -> node::CalculNode {
    let mut start_with_l_paren: bool = false;
    let mut args = resp::handler::make_null_node();

    while tokens.len() > *index as usize {
        match tokens[*index as usize].kind {
            token::TokenKind::TokenLParen => {
                start_with_l_paren = true;
            },
            token::TokenKind::TokenRParen => {
                *index += 1;
                break;
            },
            token::TokenKind::TokenName | token::TokenKind::TokenNum => {
                if start_with_l_paren {
                    args = resp::handler::make_operator_node(
                        parse_comper_op(tokens.clone(), index),
                        args.clone(),
                        node::NodeKind::NodeArgsValue
                    );
                } else {
                    eprintln!("[syntax err]: args");
                }
            },
            _ => {
                //
            }
        }

        *index += 1;
    }

    args.clone()
}

//  呼び出す値が関数か変数かを調べる
fn call_value_node(
        tokens: Vec<token::Token>,
        current_token: token::Token,
        index: &mut i32
) -> node::CalculNode {
    if tokens.len() > 1 + *index as usize && tokens[1 + *index as usize].kind == token::TokenKind::TokenLParen {
        *index += 1;
        //  関数
        let args_node = parse_func_arg(tokens, index);
        let func_head_data = node::CalculNode {
            value: current_token.lexeme.clone(),
            node_type: node::NodeKind::NodeCallFunc,
            left_node: None,
            right_node: Some(Box::new(args_node))
        };
        return func_head_data;
    } else {
        //  変数
        *index += 1;
        return resp::handler::make_value_node(
            &current_token,
            node::NodeKind::NodeCallVar,
        );
    }
}

pub fn parse_factor(tokens: Vec<token::Token>, index: &mut i32) -> node::CalculNode {
    let current_token = &tokens.clone()[*index as usize];

    match current_token.kind {
        token::TokenKind::TokenNum => {
            *index += 1;
            return resp::handler::make_value_node(
                current_token,
                node::NodeKind::NodeNum,
            );
        }
        token::TokenKind::TokenName => return call_value_node(tokens, current_token.clone(), index),
        token::TokenKind::TokenString => {
            *index += 1;
            return resp::handler::convert_value_to_node(
                current_token.lexeme.clone(),
                node::NodeKind::NodeStr,
            );
        }
        token::TokenKind::TokenLParen => {
            *index += 1; // '('をスキップ
            let node = parse_comper_op(tokens.clone(), index);
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
            return resp::handler::make_null_node();
        }
    }
}
