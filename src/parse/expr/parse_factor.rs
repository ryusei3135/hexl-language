use crate::token::token;
use crate::parse::node;
use crate::parse::resp;
use crate::parse::expr::parse_comper::parse_comper_op;

//  関数に渡す引数の値
fn parse_func_arg(tokens: Vec<token::Token>, index: &mut usize) -> node::CalculNode {
    *index += 1;
    let mut args_node = resp::handler::make_null_node();
    let mut args_list = Vec::<node::CalculNode>::new();
    let mut allow_arg: bool = true;

    while tokens.len() > *index {
        match tokens[*index].kind {
            token::TokenKind::TokenComma => {
                allow_arg = true;
            }
            token::TokenKind::TokenSpace => {}
            token::TokenKind::TokenRParen => {
                *index += 1;
                break;
            }
            _ => {
                if allow_arg {
                    args_list.push(parse_comper_op(tokens.clone(), index));
                    allow_arg = false;
                    continue;
                } else {
                    println!("[syntax err]: args");
                }
            }
        }
        *index += 1;
    }

    while args_list.len() > 0 {
        args_node = resp::handler::make_operator_node(
            args_list.pop().unwrap(),
            args_node,
            node::NodeKind::NodeArgsValue,
        );
    }

    args_node
}

/// 配列のノードを作成、実行時に配列に展開
fn make_array_node(tokens: Vec<token::Token>, index: &mut usize) -> node::CalculNode {
    let mut array_node = resp::handler::make_null_node();
    let mut can_next_be_value: bool = true;

    while tokens.len() > *index {
        match tokens[*index].kind {
            token::TokenKind::TokenComma => {
                if !can_next_be_value {
                    can_next_be_value = true;
                } else {
                    //
                }
            }
            token::TokenKind::TokenSpace => {}
            token::TokenKind::TokenLBracket => {
                if can_next_be_value {
                    *index += 1;
                    array_node = resp::handler::make_operator_node(
                        make_array_node(tokens.clone(), index),
                        array_node,
                        node::NodeKind::NodeArray,
                    );
                    can_next_be_value = false;
                    continue;
                }
            }
            _ => {
                if can_next_be_value {
                    array_node = resp::handler::make_operator_node(
                        parse_factor(tokens.clone(), index),
                        array_node,
                        node::NodeKind::NodeArray,
                    );
                    can_next_be_value = false;
                    continue;
                }
            }
        }
        *index += 1;
    }

    array_node
}

///  呼び出す値が関数か変数かを調べる
pub fn call_value_node(
        tokens: Vec<token::Token>,
        current_token: token::Token,
        index: &mut usize
) -> node::CalculNode {
    if tokens.len() > 1 + *index {
        *index += 1;
        match tokens[*index].kind {
            token::TokenKind::TokenLParen => {
                let args_node = parse_func_arg(tokens.clone(), index);
                let func_head_data = node::CalculNode {
                    value: current_token.lexeme.clone(),
                    node_type: node::NodeKind::NodeCallFunc,
                    left_node: Some(Box::new(args_node)),
                    right_node: None,
                    block: None
                };
                return func_head_data;
            }
            token::TokenKind::TokenRBracket => {
                //  TokenNameの次にLParenでもTokenDotでもないかつRBracketならそれは変数
                if tokens[(*index) - 1].kind == token::TokenKind::TokenName {
                    return resp::handler::make_value_node(
                        &current_token,
                        node::NodeKind::NodeCallVar,
                    );
                }
            }
            token::TokenKind::TokenDot => {
                *index += 1;
                let current_token_dot = tokens[*index].clone();
                let method_node = call_value_node(tokens, current_token_dot, index);

                return resp::handler::make_receiver_node(
                    current_token.lexeme.clone(),
                    method_node.clone(),
                );
            }
            token::TokenKind::TokenSpace => {
                //  TokenNameの次に空白が来たらそれは変数
                if tokens[(*index) - 1].kind == token::TokenKind::TokenName {
                    return resp::handler::make_value_node(
                        &current_token,
                        node::NodeKind::NodeCallVar,
                    );
                }
            }
            _ => println!("what is token -> {:?}", tokens[*index].kind),
        }
    } else {
        //  変数
        *index += 1;
        return resp::handler::make_value_node(
            &current_token,
            node::NodeKind::NodeCallVar,
        );
    }

    resp::handler::make_null_node()
}

pub fn parse_factor(tokens: Vec<token::Token>, index: &mut usize) -> node::CalculNode {
    let current_token = &tokens.clone()[*index];

    match current_token.kind {
        token::TokenKind::TokenNum => {
            *index += 1;
            return resp::handler::make_value_node(
                current_token,
                node::NodeKind::NodeNum,
            );
        }
        token::TokenKind::TokenLBracket => {
            *index += 1;
            return make_array_node(tokens.clone(), index);
        }
        token::TokenKind::TokenNot => {
            if tokens.len() > 1 + *index {
                *index += 1;
                return resp::handler::make_operator_node(
                    parse_factor(tokens, index),
                    resp::handler::make_null_node(),
                    node::NodeKind::NodeNot
                );
            } else {
                println!("[syntax err]: line {}", tokens[*index].line);
                panic!("");
            }
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
            if tokens[*index].kind == token::TokenKind::TokenRParen {
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
