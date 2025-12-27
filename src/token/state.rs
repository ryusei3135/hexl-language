use crate::token::token;
use crate::token::resp;

#[derive(Clone, Debug)]
pub struct LexerState {
    //  トークンの文字列を一時的に保存する変数
    pub stack_token_txt: String,
    //  直前のトークンの種類
    pub last_token_kind: token::TokenKind,
    //  直前の文字
    pub last_char: char,
    //  直前の文字の種類
    pub last_char_kind: token::CharKind,
    //  トークン本体
    pub tokens: Vec<token::Token>,

    pub line_number: usize,
}

pub struct LexerRole {
    pub processor: resp::apply::ApplyProcessor,
}