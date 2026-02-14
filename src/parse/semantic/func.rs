use super::*;

/// 引数のデータを新しく作るときに新しくデータを初期化
fn make_init_args_node(
        last_node: &Option<manager::func::FuncArgsNode>
) -> manager::func::FuncArgsNode {
    manager::func::FuncArgsNode {
        name: "[*null*]".to_string(),
        type_name: None,
        next: match last_node {
            Some(node) => Some(Box::new(node.clone())),
            None => None
        },
        multiple: None,
    }
}

//  関数の引数
fn make_args_node(
        tokens: &Vec<token::Token>,
        index: &mut usize
) -> Result<manager::func::FuncArgsNode, err_kind::ErrorsKind> {
    let mut args_node = make_init_args_node(&None);
    let mut var_status: Option<manager::variable::MultipleVar> = None;

    while tokens.len() > *index {
        match tokens[*index].kind {
            token::TokenKind::TokenVarMut => {
                if var_status == None {
                    var_status = Some(manager::variable::MultipleVar::IsMut);
                } else {
                    Err(err_kind::ErrorsKind::MultipleMutabilitySpecifiers)?;
                }
            }
            token::TokenKind::TokenVarImm => {
                if var_status == None {
                    var_status = Some(manager::variable::MultipleVar::IsImm);
                } else {
                    Err(err_kind::ErrorsKind::MultipleMutabilitySpecifiers)?;
                }
            }
            token::TokenKind::TokenName => args_node.name = tokens[*index].lexeme.clone(),
            token::TokenKind::TokenSpace => {},
            token::TokenKind::TokenRParen => {
                *index += 1;
                break;
            }
            token::TokenKind::TokenLessThan => {
                args_node.type_name = {
                    Some(
                        parse_type::parse_type_node(
                            tokens,
                            index
                        ).unwrap().value.clone()
                    )
                };
                args_node.multiple = var_status.clone();
                continue;
            }
            token::TokenKind::TokenComma => {
                args_node = make_init_args_node(&Some(args_node));
                var_status = None;
            }
            _ => {}
        }
        *index += 1;
    }

    Ok(args_node)
}

//  関数ヘッダーを作成する関数
pub fn make_func_header(
        tokens: Vec<token::Token>,
        index: &mut usize
) -> Result<manager::func::FuncNode, err_kind::ErrorsKind> {
    let mut func_start_keyword: bool = false;
    //  関数の名前がある場所を代入
    let mut func_name_index: Option<usize> = None;
    let mut args = make_init_args_node(&None);
    let mut func_ret_value_node: Option<node::CalculNode> = None;

    while tokens.len() > *index {
        match tokens[*index].kind {
            token::TokenKind::TokenName => {
                if func_start_keyword {
                    func_name_index = Some(*index);
                } else {
                    //  関数の初めに来るキーワードがまだ出てきていない
                    break;
                }
            },
            token::TokenKind::TokenSpace => {
                //  スペースは無視する
            },
            token::TokenKind::TokenLBrace => {
                //  "{"は別の関数で処理
                break;
            }
            token::TokenKind::TokenLParen => {
                args = make_args_node(&tokens, index)?;
                continue;
            },
            token::TokenKind::TokenLessThan => {
                func_ret_value_node = Some(parse_type::parse_type_node(&tokens, index)?);
            }
            token::TokenKind::TokenFuncStart => {
                func_start_keyword = true;
            }
            _ => {
                println!("{:?} func header", tokens[*index].kind);
            }
        }
        *index += 1;
    }

    if func_name_index.is_some() {
        let type_name = if func_ret_value_node.is_none() {
            "null".to_string()
        } else {
            func_ret_value_node.unwrap().value.clone()
        };
        return Ok(manager::func::FuncNode {
            name: tokens[func_name_index.unwrap()].lexeme.clone(),
            args: args,
            ret_type: type_api::change_txt_type_to_type(&type_name),
            nodes: Vec::<node::CalculNode>::new(),
        });
    } else {
        panic!("syntax err: func define");
    }
}
