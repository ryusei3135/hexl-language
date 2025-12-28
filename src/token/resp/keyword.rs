use crate::token::token;


pub fn categorize_keyword(text: &str) -> token::TokenKind {
    match text {
        "def" => token::TokenKind::TokenFuncStart,
        "use" => token::TokenKind::TokenUsePackage,
        "let" => token::TokenKind::TokenNewVar,
        _     => token::TokenKind::TokenName,
    }
}