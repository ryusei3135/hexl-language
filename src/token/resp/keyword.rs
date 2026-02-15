use crate::token::token;


fn categorize_keyword(text: &str) -> token::TokenKind {
    match text {
        "def" => token::TokenKind::TokenFuncStart,
        "use" => token::TokenKind::TokenUsePackage,
        // "let" => token::TokenKind::TokenNewVar,
        "if" => token::TokenKind::TokenIf,
        "else" => token::TokenKind::TokenElse,
        "loop" => token::TokenKind::TokenFor,
        "ret" => token::TokenKind::TokenRet,
        "in" => token::TokenKind::TokenIn,
        "mut" => token::TokenKind::TokenVarMut,
        "imm" => token::TokenKind::TokenVarImm,
        "true" => token::TokenKind::TokenBoolTrue,
        "false" => token::TokenKind::TokenBoolFalse,
        _     => token::TokenKind::TokenName,
    }
}

pub fn change_txt_for_token(tokens: &mut Vec<token::Token>) {
    for t in tokens {
        if t.kind == token::TokenKind::TokenName {
            t.kind = categorize_keyword(t.lexeme.as_str());
        }
    }
}
