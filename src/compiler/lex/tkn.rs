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
    And,
    Xor,
    Or,
    Dot,
    LAngleBracket,
    RAngleBracket,
    LBracket,
    RBracket,

    GreaterThanEq,
    LessThanEq,
    /// `==` (値の比較用の等価演算子)
    EqEq,
    NotEq,
    Not,
    /// `=>` (matchの条件/パターンの後に付ける矢印)
    Arrow,

    LShift,
    RShift,

    AddEq, // +=
    SubEq, // -=
    MulEq, // *=
    DivEq, // /=

    Inc,
    Dec,

    CompleSyn,

    /// 整数リテラル (`10` / 16進数は10進数の文字列に変換済み)
    Number(String),
    /// 小数点を含む数値リテラル (`3.14`)。値はソース上の文字列のまま保持する。
    /// `.` の前後に数字が必要で、`1..2` (範囲) や `a.b` (メンバー) とは区別される。
    Float(String),
    Name(String),
    Str(String),

    ModPathTkn,
    RangeTkn,

    KeyWordRet,
    KeyWordCond,
    KeyWordLoop,
    KeyWordPub,
    KeyWordStruct,
    KeyWordVariant,
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

    KeyWordWhere,
    KeyWordAllow,
    KeyWordElse,
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
