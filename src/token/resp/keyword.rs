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
        } else if t.kind == token::TokenKind::TokenString {
            if t.lexeme.chars().count() < 2 {
                t.lexeme.clear();
                return;
            }

            let start = t.lexeme.char_indices().nth(1).unwrap().0;
            let end = t.lexeme.char_indices().rev().nth(1).unwrap().0 + 1;

            t.lexeme.drain(..start); // 先頭削除
            t.lexeme.drain(end - start..); // 末尾削除（位置調整）
        }
    }
}
