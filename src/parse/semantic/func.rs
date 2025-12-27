use crate::token::token;

use crate::parse::node;


fn make_args_node(tokens: Vec<token::Token>, index: &mut i32) {
    *index += 1;

    while tokens.len() > *index as usize {
        *index += 1;
    }
}

//  関数ヘッダーを作成する関数
pub fn make_func_header(tokens: Vec<token::Token>, index: &mut i32) -> node::FuncNode {
    let mut func_start_keyword: bool = false;
    //  関数の名前がある場所を代入
    let mut func_name_index: i32 = -1;

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
                make_args_node(tokens.clone(), index);
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
            nodes: Vec::<node::CalculNode>::new(),
        };
    } else {
        panic!("syntax err: func define");
    }
}