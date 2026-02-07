

#[derive(Clone, Debug, PartialEq)]
pub enum CharKind {
    CharNum,
    CharAlpha,
    CharSymbol,
    CharSpace,
    // CharNull,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TokenKind {
    TokenNum,
    TokenName,
    TokenSpace,
    TokenString,

    TokenAdd,
    TokenSub,
    TokenMul,
    TokenDiv,

    TokenEqTo,
    TokenNotEqTo,
    TokenLessThanOrEqualTo,
    TokenGreaterThanOrEqualTo,

    TokenLParen,
    TokenRParen,
    TokenLBrace,    // "{"
    TokenRBrace,    // "}"
    TokenAssign,    //  "="
    TokenSpacer,    //  ":"
    TokenLessThan,  //  "<"
    TokenGreaterThan,// ">"
    TokenNot,       //  "!"
    TokenComma,     //  ","
    TokenScope,     //  "::"
    TokenLBracket,  //  "["
    TokenRBracket,  //  "]"
    TokenRangeOp,   //  ".."
    TokenDot,

    //  キーワード
    TokenFuncStart, // "def"
    TokenUsePackage,// "use"
    TokenNewVar,    // "let"
    TokenIf,        // "if"
    TokenElse,      // "else"
    TokenFor,       // "for"
    TokenIn,        // "in"
    TokenRet,
    TokenEOF,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub lexeme: String,
    pub line: usize,
}

impl Token {
    pub fn connect(&mut self, txt: &str) -> bool {
        self.lexeme.push_str(txt);
        true
    }

    pub fn change(&mut self, kind: TokenKind) -> &mut Self {
        self.kind = kind;
        self
    }
}
