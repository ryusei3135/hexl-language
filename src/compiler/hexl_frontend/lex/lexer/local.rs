/// asciiに対応したテーブルを作成する
pub fn make_char_table() -> [CharKind; 256] {
    let mut table: [CharKind; 256] = [CharKind::Other; 256];

    table[0x09..=0x0D].fill(CharKind::Space);
    table[0x20] = CharKind::Space;
    table[0x21..=0x2F].fill(CharKind::Op);
    table[0x30..=0x39].fill(CharKind::Num);
    table[0x3a..=0x3f].fill(CharKind::Op);
    table[0x41..=0x5A].fill(CharKind::Name);
    table[0x5B..=0x60].fill(CharKind::Op);
    table[0x5F] = CharKind::Name;
    table[0x61..=0x7A].fill(CharKind::Name);
    table[0x7B..=0x7E].fill(CharKind::Op);

    table[0x0A] = CharKind::Ln;
    table
}

#[derive(Clone, Debug, PartialEq, Copy)]
pub enum CharKind {
    Op,
    Num,
    Name,
    Space,
    Ln,
    Other,
}

impl CharKind {
    pub fn flag(self) -> Option<GenFlag> {
        match self {
            Self::Name => Some(GenFlag::Name),
            Self::Num => Some(GenFlag::Number),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Copy)]
pub enum GenFlag {
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
    And,
    Xor,
    Or,
    Dot,
    LAngleBracket,
    RAngleBracket,
    LBracket,
    RBracket,
    Not,
    Surplus,
    Question,

    Inversion,
    CompleSyn,

    Number,
    Name,
    Str,
}

/// `check_stkable_chr` の戻り値。
#[derive(Clone, Debug, PartialEq, Copy)]
pub enum StkResult {
    /// 現在の文字をスタックに積んでよい
    Stackable,
    /// スタックを不可とし、トークンを生成してから現在の文字を積む
    GenTkn,
    /// 現在の文字は処理済み（閉じクォートなど）。トークンを生成し、
    /// かつ現在の文字は `chr_stk` に積まない。`last_kind` も
    /// （ひとつのトークンが完結した区切りとして）None にリセットする。
    Consumed,
}
