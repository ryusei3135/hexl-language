use crate::token::token;

use crate::parse::node;
use crate::parse::expr::parse_type;


fn make_args_node(tokens: Vec<token::Token>, index: &mut i32) -> Vec<node::FuncArgsNode> {
    let mut args = Vec::<node::FuncArgsNode>::new();
    let mut arg_type: Option<String> = None;
    let mut arg_name: Option<String> = None;

    if tokens[*index as usize].kind == token::TokenKind::TokenLParen {
        *index += 1;
    }

    while tokens.len() > *index as usize {
        match tokens[*index as usize].kind {
            token::TokenKind::TokenLessThan => {
                arg_type = Some(parse_type::parse_type_node(tokens.clone(), index).value.clone());
            },
            token::TokenKind::TokenName => {
                arg_name = Some(tokens[*index as usize].lexeme.clone());
            },
            token::TokenKind::TokenRParen => {
                *index += 1;
                return args;
            }
            _ => panic!("[syntax err] func args"),
        }

        if arg_type != None && arg_name != None {
            args.push(
                node::FuncArgsNode {
                    name: arg_name.unwrap(),
                    type_name: arg_type.unwrap()
                }
            );
            arg_name = None;
            arg_type = None;
        }

        *index += 1;
    }

    args
}

//  関数ヘッダーを作成する関数
pub fn make_func_header(tokens: Vec<token::Token>, index: &mut i32) -> node::FuncNode {
    let mut func_start_keyword: bool = false;
    //  関数の名前がある場所を代入
    let mut func_name_index: i32 = -1;
    let mut args = Vec::<node::FuncArgsNode>::new();

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
        return node::FuncNode {
            name: tokens[func_name_index as usize].lexeme.clone(),
            args: args,
            nodes: Vec::<node::CalculNode>::new(),
        };
    } else {
        panic!("syntax err: func define");
    }
}