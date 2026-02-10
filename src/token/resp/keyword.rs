use crate::token::token;


pub fn categorize_keyword(text: &str) -> token::TokenKind {
    match text {
        "def" => token::TokenKind::TokenFuncStart,
        "use" => token::TokenKind::TokenUsePackage,
        // "let" => token::TokenKind::TokenNewVar,
        "if" => token::TokenKind::TokenIf,
        "else" => token::TokenKind::TokenElse,
        "for" => token::TokenKind::TokenFor,
        "ret" => token::TokenKind::TokenRet,
        "in" => token::TokenKind::TokenIn,
        _     => token::TokenKind::TokenName,
    }
}
