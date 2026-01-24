use super::*;


fn make_init_args_node(last_node: Option<manager::func::FuncArgsNode>) -> manager::func::FuncArgsNode {
    manager::func::FuncArgsNode {
        name: "[*null*]".to_string(),
        type_name: None,
        next: match last_node {
            Some(node) => Some(Box::new(node)),
            None => None
        }
    }
}

//  関数の引数
fn make_args_node(tokens: Vec<token::Token>, index: &mut i32) -> manager::func::FuncArgsNode {
    let mut args_node = make_init_args_node(None);

    while tokens.len() > *index as usize {
        match tokens[*index as usize].kind {
            token::TokenKind::TokenName => args_node.name = tokens[*index as usize].lexeme.clone(),
            token::TokenKind::TokenSpace => {},
            token::TokenKind::TokenRParen => {
                *index += 1;
                break;
            }
            token::TokenKind::TokenLessThan => {
                args_node.type_name = {
                    Some(
                        parse_type::parse_type_node(
                            tokens.clone(),
                            index
                        )
                    )
                };
                continue;
            }
            token::TokenKind::TokenComma =>
                args_node = make_init_args_node(Some(args_node)),
            _ => {}
        }
        *index += 1;
    }

    args_node
}

//  関数ヘッダーを作成する関数
pub fn make_func_header(tokens: Vec<token::Token>, index: &mut i32) -> manager::func::FuncNode {
    let mut func_start_keyword: bool = false;
    //  関数の名前がある場所を代入
    let mut func_name_index: i32 = -1;
    let mut args = make_init_args_node(None);
    let mut func_ret_value_node: Option<node::CalculNode> = None;

    while tokens.len() > *index as usize {
        match tokens[*index as usize].kind {
            token::TokenKind::TokenName => {
                if func_start_keyword {
                    func_name_index = *index;
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
                args = make_args_node(tokens.clone(), index);
                continue;
            },
            token::TokenKind::TokenLessThan => {
                func_ret_value_node = Some(parse_type::parse_type_node(tokens.clone(), index));
            }
            token::TokenKind::TokenFuncStart => {
                func_start_keyword = true;
            }
            _ => {
                println!("{:?}", tokens[*index as usize].kind);
            }
        }
        *index += 1;
    }

    //  もし、"func_name_index"が0未満なら、構文エラー
    if func_name_index >= 0 {
        return manager::func::FuncNode {
            name: tokens[func_name_index as usize].lexeme.clone(),
            args: args,
            ret_value_type: type_api::change_txt_type_to_type(
                if func_ret_value_node.is_none() {
                    "null".to_string()
                } else {
                    func_ret_value_node.unwrap().value.clone()
                }
            ),
            nodes: Vec::<node::CalculNode>::new(),
        };
    } else {
        panic!("syntax err: func define");
    }
}
