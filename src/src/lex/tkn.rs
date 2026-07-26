


#[derive(Clone, Debug, PartialEq)]
pub enum Tkn {
    Add,
    Sub,
    Mul,
    Div,
    Equal,
    LParen,
    RParen,
    LBrace,
    RBrace,
    Comma,
    Colon,
    Or,
    Dot,
    LAngleBracket,
    RAngleBracket,
    LBracket,
    RBracket,

    CompleSyn,

    Number(String),
    Name(String),
    Str(String),

    ModPathTkn,

    KeyWordRet,
    KeyWordMatch,
    KeyWordLoop,
    KeyWordPub,
    KeyWordStruct,
    KeyWordEnum,
    KeyWordConst,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LocatedTkn {
    pub tkn: Tkn,
    pub pos: usize,
    pub line: usize,
}
