use crate::token::categorize;
use crate::token::token;
use crate::token::state;
use crate::token::token_passes::judge;

pub struct ApplyProcessor {}


impl ApplyProcessor {
    pub fn new() -> Self {
        ApplyProcessor {}
    }

    pub fn emit_token(
            &mut self, 
            state: &mut state::LexerState,
            c: char
    ) {
        //  直前のトークンの種類を確定させる
        let token_kind = categorize::categorize_token(
            &state.stack_token_txt,
            state.last_char_kind.clone(),
        );

        //  トークンを追加
        state.tokens.push(
            token::Token {
                kind: token_kind.clone(),
                lexeme: state.stack_token_txt.clone(),
                line: state.line_number,
            }
        );
        //  次のトークンの準備
        state.stack_token_txt.clear();
        state.last_token_kind = token_kind.clone();

        judge::judge_merge_token(&mut state.tokens);
    }

    pub fn combine_char(
            &self, 
            state: &mut state::LexerState, 
            char_kind: token::CharKind, 
            c: char
    ) {
        state.stack_token_txt.push(c);
        state.last_char_kind = char_kind.clone();
        state.last_char = c;
    }
}