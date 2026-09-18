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
    // %
    Surplus,
    Or,
    Dot,
    LAngleBracket,
    RAngleBracket,
    LBracket,
    RBracket,

    /// `==` (値の比較用の等価演算子)
    EqEq,
    NotEq,
    Not,
    /// `=>` (matchの条件/パターンの後に付ける矢印)
    Arrow,

    CompleSyn,

    Number(String),
    Name(String),
    Str(String),

    ModPathTkn,
    RangeTkn,

    KeyWordRet,
    KeyWordCond,
    KeyWordLoop,
    KeyWordPub,
    KeyWordStruct,
    KeyWordEnum,
    KeyWordConst,
    KeyWordStatic,
    /// mut
    KeyWordMut,
    /// `Self` (メゾット内で自身の構造体を表す予約語)
    KeyWordSelf,

    KeyWordMust,
    KeyWordOf,

    KeyWordContinue,
    KeyWordBreak,
}

impl Tkn {
    pub fn unwrap_name(&self) -> String {
        if let Self::Name(val) = self.clone() {
            val
        } else {
            panic!();
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LocatedTkn {
    pub tkn: Tkn,
    pub pos: usize,
    pub line: usize,
}
