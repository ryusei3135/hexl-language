use crate::token::categorize;
use crate::token::check;
use crate::token::resp;
use crate::token::state;
use crate::token::token;


pub struct Tokenizer {
    pub lexer_state: state::LexerState,
    pub lexer_role: state::LexerRole,
}

impl Tokenizer {
    pub fn new() -> Self {
        Tokenizer {
            lexer_state: state::LexerState {
                stack_token_txt: String::new(),
                last_token_kind: token::TokenKind::TokenEOF,
                last_char: ' ',
                last_char_kind: token::CharKind::CharSpace,
                tokens: Vec::<token::Token>::new(),
                line_number: 0,
            },
            lexer_role: state::LexerRole {
                processor: resp::apply::ApplyProcessor::new(),
            },
        }
    }

    pub fn init(&mut self) {
        //  トークンの情報を初期化
        self.lexer_state = state::LexerState {
            stack_token_txt: String::new(),
            last_token_kind: token::TokenKind::TokenEOF,
            last_char: ' ',
            last_char_kind: token::CharKind::CharSpace,
            tokens: Vec::<token::Token>::new(),
            line_number: 0,
        };
    }

    pub fn make_token(&mut self, line: String, line_number: usize) -> Vec<token::Token> {
        self.lexer_state.line_number = line_number;

        for chr in line.chars() {
            let char_kind = categorize::categorize_char(chr);

            // 前回の文字の種類と今の文字の種類が同じか調べる
            if check::same_char_kind(&self.lexer_state, &char_kind) {
                // ドット同士をつなげトークンを確定させる
                if self.lexer_state.last_char == '.' && chr == '.' {
                    self.lexer_role.processor.combine_char(
                        &mut self.lexer_state,
                        char_kind.clone(),
                        chr
                    );
                    self.lexer_role.processor.emit_token(&mut self.lexer_state);
                    continue;
                }
                //  もしどっちも記号ならトークンを確定させる
                if check::is_symbol(&self.lexer_state.last_char_kind, &char_kind) {
                    self.lexer_role.processor.emit_token(&mut self.lexer_state);
                }
            } else {
                self.lexer_role.processor.emit_token(&mut self.lexer_state);
            }
            self.lexer_role.processor.combine_char(
                &mut self.lexer_state,
                char_kind.clone(),
                chr
            );
        }

        self.lexer_role.processor.emit_token(&mut self.lexer_state);

        let results = self.lexer_state.tokens.clone();
        self.lexer_state.tokens.clear();
        return results;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token::tokenizer;

    #[test]
    fn check_float_token() {
        let mut lexer = tokenizer::Tokenizer::new();
        let float_token = lexer.make_token("1.1".to_string(), 0);

        assert_eq!(float_token[0].kind, token::TokenKind::TokenFloat);
    }
}
