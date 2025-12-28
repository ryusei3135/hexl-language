use crate::token::token;


pub fn make_use_package_node(tokens: Vec<token::Token>, index: &mut i32) {
    let mut first_use_package_token: bool = false;
    let mut package_name: String;

    while tokens.len() > *index as usize {
        match tokens[*index as usize].kind {
            token::TokenKind::TokenUsePackage => {
                if !first_use_package_token {
                    first_use_package_token = true;
                } else {
                    println!("[syntax err]: invalid syntax");
                    println!("  line {}", tokens[*index as usize].line);
                }
            },
            token::TokenKind::TokenSpace => {
                //
            },
            token::TokenKind::TokenName => {
                //  use_packageトークンの後に、文字トークンが来ないと構文エラー
                if first_use_package_token {
                    package_name = tokens[*index as usize].lexeme.clone();
                }
            },
            _ => {
                //
            }
        }

        *index += 1;
    }
}