use crate::token::token;


pub fn categorize_keyword(text: &str) -> token::TokenKind {
    match text {
        "def" => token::TokenKind::TokenFuncStart,
        _     => token::TokenKind::TokenName,
    }
}