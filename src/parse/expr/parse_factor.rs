use super::*;

//  関数に渡す引数の値
fn parse_func_arg(
        tokens: &Vec<token::Token>,
        index: &mut usize
) -> Result<node::CalculNode, err_kind::ErrorsKind> {
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
                    args_list.push(parse_comper_op(tokens.clone(), index)?);
                    allow_arg = false;
                    continue;
                } else {
                    Err(err_kind::ErrorsKind::MissingCommaBetweenArguments)?;
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
            &tokens[0].line
        );
    }

    Ok(args_node)
}

/// 配列のノードを作成、実行時に配列に展開
fn make_array_node(
        tokens: &Vec<token::Token>,
        index: &mut usize
) -> Result<node::CalculNode, err_kind::ErrorsKind> {
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
                        make_array_node(tokens, index)?,
                        array_node,
                        node::NodeKind::NodeArray,
                        &tokens[0].line
                    );
                    can_next_be_value = false;
                    continue;
                }
            }
            _ => {
                if can_next_be_value {
                    array_node = resp::handler::make_operator_node(
                        parse_factor(tokens, index)?,
                        array_node,
                        node::NodeKind::NodeArray,
                        &tokens[0].line
                    );
                    can_next_be_value = false;
                    continue;
                }
            }
        }
        *index += 1;
    }

    Ok(array_node)
}

fn make_range_node(
        tokens: &Vec<token::Token>,
        index: &mut usize
) -> node::CalculNode {
    let mut node = resp::handler::make_null_node();
    node.left_node = Some(
        Box::new(
            resp::handler::convert_value_to_node(
                tokens[*index].lexeme.clone(),
                node::NodeKind::NodeNum
            )
        )
    );
    node.right_node = Some(
        Box::new(
            resp::handler::convert_value_to_node(
                tokens[*index + 2].lexeme.clone(),
                node::NodeKind::NodeNum
            )
        )
    );
    node.node_type = node::NodeKind::NodeRangeOp;
    *index += 3;
    node
}

///  呼び出す値が関数か変数かを調べる
pub fn call_value_node(
        tokens: &Vec<token::Token>,
        current_token: token::Token,
        index: &mut usize
) -> Result<node::CalculNode, err_kind::ErrorsKind> {
    if tokens.len() > 1 + *index {
        *index += 1;
        match tokens[*index].kind {
            token::TokenKind::TokenLParen => {
                let args_node = parse_func_arg(tokens, index)?;
                let func_head_data = node::CalculNode {
                    value: current_token.lexeme.clone(),
                    node_type: node::NodeKind::NodeCallFunc,
                    left_node: Some(Box::new(args_node)),
                    right_node: None,
                    block: None
                };
                return Ok(func_head_data);
            }
            token::TokenKind::TokenRBracket => {
                //  TokenNameの次にLParenでもTokenDotでもないかつRBracketならそれは変数
                if tokens[(*index) - 1].kind == token::TokenKind::TokenName {
                    return Ok(resp::handler::make_value_node(
                        &current_token,
                        node::NodeKind::NodeCallVar,
                    ));
                }
            }
            token::TokenKind::TokenDot => {
                *index += 1;
                let current_token_dot = tokens[*index].clone();
                let method_node = call_value_node(tokens, current_token_dot, index)?;

                return Ok(resp::handler::make_receiver_node(
                    current_token.lexeme.clone(),
                    method_node.clone(),
                ));
            }
            token::TokenKind::TokenScope => {
                *index += 1;
                let current_token_dot = tokens[*index].clone();
                let module_node = call_value_node(tokens, current_token_dot, index)?;

                return Ok(resp::handler::make_call_module(
                    current_token.lexeme.clone(),
                    module_node.clone(),
                ));
            }
            token::TokenKind::TokenSpace | token::TokenKind::TokenRParen => {
                //  TokenNameの次に空白が来たらそれは変数
                if tokens[(*index) - 1].kind == token::TokenKind::TokenName {
                    return Ok(resp::handler::make_value_node(
                        &current_token,
                        node::NodeKind::NodeCallVar,
                    ));
                }
            }
            _ => println!("what is token -> {:?}", tokens[*index].kind),
        }
    } else {
        //  変数
        *index += 1;
        return Ok(resp::handler::make_value_node(
            &current_token,
            node::NodeKind::NodeCallVar,
        ));
    }

    Ok(resp::handler::make_null_node())
}

pub fn parse_factor(
        tokens: &Vec<token::Token>,
        index: &mut usize
) -> Result<node::CalculNode, err_kind::ErrorsKind> {
    let current_token = &tokens.clone()[*index];

    match current_token.kind {
        token::TokenKind::TokenNum => {
            if tokens.len() > *index + 1 {
                if tokens[*index + 1].kind == token::TokenKind::TokenRangeOp {
                    return Ok(make_range_node(&tokens, index));
                }
            }
            *index += 1;
            return Ok(resp::handler::make_value_node(
                current_token,
                node::NodeKind::NodeNum,
            ));
        }
        token::TokenKind::TokenBoolTrue => {
            return Ok(resp::handler::convert_value_to_node(
                current_token.lexeme.clone(),
                node::NodeKind::NodeBoolTrue
            ));
        }
        token::TokenKind::TokenBoolFalse => {
            return Ok(resp::handler::convert_value_to_node(
                current_token.lexeme.clone(),
                node::NodeKind::NodeBoolFalse
            ));
        }
        token::TokenKind::TokenFloat => {
            *index += 1;
            return Ok(resp::handler::make_value_node(
                current_token,
                node::NodeKind::NodeFloat,
            ));
        }
        token::TokenKind::TokenLBracket => {
            *index += 1;
            return make_array_node(tokens, index);
        }
        token::TokenKind::TokenNot => {
            if tokens.len() > 1 + *index {
                *index += 1;
                return Ok(resp::handler::make_operator_node(
                    parse_factor(tokens, index)?,
                    resp::handler::make_null_node(),
                    node::NodeKind::NodeNot,
                    &tokens[0].line
                ));
            } else {
                println!("[syntax err]: line {}", tokens[*index].line);
                panic!("");
            }
        }
        token::TokenKind::TokenName => return call_value_node(tokens, current_token.clone(), index),
        token::TokenKind::TokenString => {
            *index += 1;
            return Ok(resp::handler::convert_value_to_node(
                current_token.lexeme.clone(),
                node::NodeKind::NodeStr,
            ));
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
            return Ok(resp::handler::make_null_node());
        }
    }
}
