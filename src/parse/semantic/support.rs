use crate::token::token;

use crate::parse::node;


pub fn make_support_node(tokens: Vec<token::Token>, index: &mut i32) {
    let mut first_support_token: bool = false;

    while tokens.len() > *index as usize {
        match tokens[*index as usize].kind {
            token::TokenKind::TokenSupport => {
                if !first_support_token {
                    first_support_token = true;
                } else {
                    println!("[syntax err]: invalid syntax");
                    println!("  line {}", tokens[*index as usize].line);
                }
            },
            token::TokenKind::TokenSpace => {
                //
            }
            token::TokenKind::TokenName => {
                //  supportトークンの後に、文字トークンが来ないと構文エラー
                if first_support_token {
                    //
                }
            }
        }

        *index += 1;
    }
}