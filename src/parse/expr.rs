use crate::token::token;

use crate::parse::node;
use crate::parse::resp;


fn parse_factor(tokens: Vec<token::Token>, index: &mut i32) -> node::CalculNode {
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

fn parse_trim(tokens: Vec<token::Token>, index: &mut i32) -> node::CalculNode {
    let mut node = parse_factor(tokens.clone(), index);

    while tokens.len() as i32 > *index {
        match tokens[*index as usize].kind {
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

pub fn parse_expr(tokens: Vec<token::Token>, index: &mut i32) -> node::CalculNode {
    let mut node = parse_trim(tokens.clone(), index);

    while tokens.len() as i32 > *index {
        match tokens[*index as usize].kind {
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
                let operator = &tokens[*index as usize];
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

pub fn parse_assign(tokens: Vec<token::Token>, index: &mut i32) -> node::CalculNode {
    let mut first_var_name: bool = false;
    let mut node = resp::handler::make_value_node(
        &tokens[0],
        node::NodeKind::NodeVarName,
    );

    while tokens.len() > *index as usize {
        match tokens[*index as usize].kind {
            token::TokenKind::TokenAssign => {
                if first_var_name {
                    *index += 1;
                    let right = parse_expr(tokens.clone(), index);
                    node = resp::handler::make_operator_node(
                        node,
                        right,
                        node::NodeKind::NodeAssignVar,
                    );
                }
            },
            token::TokenKind::TokenSpace => {
                *index += 1;
            },
            token::TokenKind::TokenName => {
                if !first_var_name {
                    node = resp::handler::make_value_node(
                        &tokens[*index as usize],
                        node::NodeKind::NodeVarName,
                    );
                    first_var_name = true;
                    *index += 1;
                } else {
                    break;
                }
            }
            _ => {
                break;
            }
        }
    }

    return node;
}