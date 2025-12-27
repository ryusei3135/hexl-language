use crate::token::state::LexerState;
use crate::token::token;


pub fn same_char_kind(lexer_state: LexerState, char_kind: token::CharKind) -> bool {
    if lexer_state.last_char_kind == char_kind {
        return true;
    } else if lexer_state.stack_token_txt.is_empty() {
        return true;
    }

    false
}

pub fn is_parlen(last_char: char, c: char) -> bool {
    let is_last_parlen = last_char == '(' || last_char == ')';
    let is_parlen = c == '(' || c == ')';
    
    if is_parlen && is_last_parlen {
        return true;
    }
    false
}