

#[derive(Clone, Debug, PartialEq)]
pub enum CharKind {
    CharNum,
    CharAlpha,
    CharSymbol,
    CharSpace,
    CharNull,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TokenKind {
    TokenNum,
    TokenName,
    TokenSpace,

    TokenAdd,
    TokenSub,
    TokenMul,
    TokenDiv,

    TokenLParen,
    TokenRParen,
    TokenLBrace,
    TokenRBrace,
    TokenAssign,    //  "="
    TokenVarType,   //  ":"
    TokenLessThan,  //  <
    TokenGreaterThan,// >

    //  キーワード
    TokenFuncStart, // "def"
    TokenUsePackage,// "use"
    TokenNewVar,    // "let"
    TokenEOF,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub lexeme: String,
    pub line: usize,
}