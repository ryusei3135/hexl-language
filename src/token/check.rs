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

pub fn is_symbol(last_char_kind: token::CharKind, char_kind: token::CharKind) -> bool {

    if last_char_kind == token::CharKind::CharSymbol && char_kind == token::CharKind::CharSymbol {
        return true;
    }
    false
}