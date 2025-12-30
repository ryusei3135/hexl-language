use crate::token::token;


fn connect_with_name_token(
        tokens: &mut Vec<token::Token>,
        now_token: token::Token
) -> bool {
    if now_token.kind == token::TokenKind::TokenNum {
        tokens.last_mut().unwrap().lexeme.push_str(&now_token.lexeme);
    } else if now_token.lexeme == "_" {
        tokens.last_mut().unwrap().lexeme.push_str(&now_token.lexeme);
    } else if now_token.kind == token::TokenKind::TokenName {
        tokens.last_mut().unwrap().lexeme.push_str(&now_token.lexeme);
    } else {
        return false;
    }
    true
}

fn connect_minus_token(
        tokens: &mut Vec<token::Token>,
        now_token: token::Token
) -> bool {
    if tokens.last().unwrap().lexeme.chars().next() == Some('-') {
            //  トークン"-"が最初に来た場合負の数になる
        if now_token.kind == token::TokenKind::TokenNum {
            tokens.last_mut().unwrap().kind = token::TokenKind::TokenNum;
            tokens.last_mut().unwrap().lexeme.push_str(&now_token.lexeme);
        } else {
            return false;
        }
        return true;
    }

    false
}

mod str_literal {
    use crate::token::token;
    //  トークンが文字列リテラルとして代入可能か調べる
    pub fn can_extend_str_literal(now_token: token::Token) -> bool {
        if now_token.lexeme.chars().last() == Some('"') {
            if now_token.lexeme.len() >= 2 {
                return false;
            }
        } else if now_token.lexeme.chars().next() != Some('"') {
            //  文字列リテラルが終了した
            return false;
        }

        true
    }
}

fn connect_string_literal(
        tokens: &mut Vec<token::Token>,
        now_token: token::Token
) -> bool {
    if str_literal::can_extend_str_literal(tokens.last().unwrap().clone()) {
        tokens.last_mut().unwrap().lexeme.push_str(&now_token.lexeme);
        tokens.last_mut().unwrap().kind = token::TokenKind::TokenString;
        return true;
    }

    false
}

fn connect_comper_op(
        tokens: &mut token::Token,
        now_token: token::Token
) -> bool {
    return match tokens.kind {
        token::TokenKind::TokenAssign => tokens.change(token::TokenKind::TokenEqTo).connect(&now_token.lexeme),
        token::TokenKind::TokenNot => tokens.change(token::TokenKind::TokenNotEqTo).connect(&now_token.lexeme),
        _ => false,
    };
}

pub fn judge_merge_token(tokens: &mut Vec<token::Token>) -> bool {
    if tokens.len() > 2 {
        let last_token = tokens.pop().unwrap();

        let mut result = match tokens.last().unwrap().kind {
            token::TokenKind::TokenName => connect_with_name_token(tokens, last_token.clone()),
            token::TokenKind::TokenSub => connect_minus_token(tokens, last_token.clone()),
            token::TokenKind::TokenString => connect_string_literal(tokens, last_token.clone()),
            _ => false,
        };
        if !result {
            result = match last_token.kind {
                token::TokenKind::TokenAssign => connect_comper_op(&mut tokens.last_mut().unwrap(), last_token.clone()),
                _ => false,
            };
        }
        if !result {
            tokens.push(last_token);
        }
    }

    false
}
