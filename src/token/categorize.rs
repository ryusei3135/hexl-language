use crate::token::token;
use crate::token::resp;


//  文字を種類ごとに分ける関数
pub fn categorize_char(c: char) -> token::CharKind {
    if c.is_alphabetic() {
        return token::CharKind::CharAlpha;
    } else if c.is_numeric() {
        return token::CharKind::CharNum;
    } else if c.is_whitespace() {
        return token::CharKind::CharSpace;
    } else {
        return token::CharKind::CharSymbol;
    }
}

//  記号の文字列を分類する関数
pub fn categorize_symbol(symbol: &str) -> token::TokenKind {
    match symbol {
        "+" => token::TokenKind::TokenAdd,
        "-" => token::TokenKind::TokenSub,
        "*" => token::TokenKind::TokenMul,
        "/" => token::TokenKind::TokenDiv,
        "(" => token::TokenKind::TokenLParen,
        ")" => token::TokenKind::TokenRParen,
        "{" => token::TokenKind::TokenLBrace,
        "}" => token::TokenKind::TokenRBrace,
        "=" => token::TokenKind::TokenAssign,
        ":" => token::TokenKind::TokenVarType,
        _   => {
            println!("what is this symbol?: {}", symbol);
            token::TokenKind::TokenEOF
        },
    }
}

//  トークンを種類ごとに分ける関数
pub fn categorize_token(token: &str, kind: token::CharKind) -> token::TokenKind {
    match kind {
        token::CharKind::CharNum => token::TokenKind::TokenNum,
        token::CharKind::CharAlpha => {
            //  もしキーワードならキーワードとして分類する
            return resp::keyword::categorize_keyword(token);
        },
        //  文字の種類が記号の場合は、記号ごとに分類する
        token::CharKind::CharSymbol  => categorize_symbol(token),
        token::CharKind::CharSpace => {
            token::TokenKind::TokenSpace
        },
        _ => {
            println!("what is this text?: {}", token);
            token::TokenKind::TokenEOF
        },
    }
}