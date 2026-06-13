use crate::err::{self, *};


mod local {
    /// asciiに対応したテーブルを作成する
    pub fn make_char_table() -> [CharKind; 256] {
        let mut table: [CharKind; 256] = [ const { CharKind::Other }; 256 ];

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
        table[0x0D] = CharKind::Ln;
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
        LAngleBracket,
        RAngleBracket,

        Number,
        Name,
        Str,
    }
}

use local::*;


// ===== ここから公開API =====

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
    LAngleBracket,
    RAngleBracket,

    Number(String),
    Name(String),
    Str(String),
    KeyWord_Ret,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LocatedTkn {
    pub tkn: Tkn,
    pub pos: usize,
    pub line: usize,
}


const STACKABLE: bool = true;
const UNSTACKABLE: bool = false;

pub struct Lexer {
    last_kind: Option<CharKind>,
    gen_flag: Option<GenFlag>,
    chr_stk: String,
    pub gen_tkns: Vec<LocatedTkn>,
}

impl Lexer {
    pub fn new() -> Self {
        Self {
            last_kind: None,
            gen_flag: None,
            chr_stk: String::new(),
            gen_tkns: Vec::new()
        }
    }

    pub fn analy(&mut self, content: &String) -> Result<(), err::Errs> {
        self.gen_tkns = Vec::new();
        self.chr_stk = String::new();
        self.last_kind = None;
        self.gen_flag = None;
        const GEN_TKN: bool = false;
        let char_table: [CharKind; 256] = make_char_table();

        let mut chr_counter: usize = 1;
        let mut tkn_start: usize = 1;
        let mut line_counter: usize = 1;

        for chr in content.chars() {
            let curr_kind = &char_table[chr as usize];
            
            // もし*`last_kind`*がNoneなら、現在の種類同士を比較し、条件をfalseにする
            // もし文字の種類が記号の場合必ず実行する
            if self.last_kind.as_ref().unwrap_or(curr_kind) != curr_kind || curr_kind == &CharKind::Op {
                // スタック可能か調べ不可能ならトークンを生成
                if self.check_stkable_chr(&curr_kind, &chr) == GEN_TKN {
                    if let Ok(tkn) = self.gen_tkn(&tkn_start, &line_counter) {
                        tkn_start = chr_counter.clone();
                        self.gen_tkns.push(tkn);
                    }
                    self.chr_stk.clear();
                    self.gen_flag = None;
                }
            }

            if self.gen_flag.is_none() {
                match (self.last_kind.unwrap_or(CharKind::Other), curr_kind) {
                    (CharKind::Op | CharKind::Space, CharKind::Name) => {
                        self.over_write_flag::<true, STACKABLE>(GenFlag::Name);
                    }
                    (CharKind::Op | CharKind::Other | CharKind::Space, CharKind::Num) => {
                        self.over_write_flag::<true, STACKABLE>(GenFlag::Number);
                    }
                    (_, _) => {},
                }
            }
            // 改行が来たので、lineをインクリメント
            if curr_kind == &CharKind::Ln {
                line_counter += 1;
            }

            self.chr_stk.push(chr);
            self.last_kind = Some(*curr_kind);
            chr_counter += 1;
        }
        self.check_stkable_chr(&CharKind::Other, &'\0');

        if self.gen_flag.is_some() {
            self.gen_tkns.push(
                self.gen_tkn(&tkn_start, &line_counter)
                    .map_err(|v| v.gen(&line_counter, &tkn_start))
            );
        }

        Ok(())
    }

    /// トークンを生成しOptionで返す
    fn gen_tkn(
        &self,
        chr_counter: &usize,
        line_counter: &usize
    ) -> Result<LocatedTkn, err::ErrKind> {
        if let Some(ref flag) = self.gen_flag {
            let tkn = match flag {
                GenFlag::Add => Tkn::Add,
                GenFlag::Sub => Tkn::Sub,
                GenFlag::Mul => Tkn::Mul,
                GenFlag::Div => Tkn::Div,
                GenFlag::Equal => Tkn::Equal,
                GenFlag::LParen => Tkn::LParen,
                GenFlag::RParen => Tkn::RParen,
                GenFlag::LBrace => Tkn::LBrace,
                GenFlag::RBrace => Tkn::RBrace,
                GenFlag::Comma => Tkn::Comma,
                GenFlag::Colon => Tkn::Colon,
                GenFlag::LAngleBracket => Tkn::LAngleBracket,
                GenFlag::RAngleBracket => Tkn::RAngleBracket,

                GenFlag::Number => {
                    if self.chr_stk.starts_with("0x") {
                        let num = u32::from_str_radix(self.chr_stk.clone().trim_start_matches("0x"), 16).unwrap();
                        Tkn::Number(num.to_string())
                    } else {
                        Tkn::Number(self.chr_stk.clone())
                    }
                }
                GenFlag::Name => {
                    match self.chr_stk.as_str() {
                        "ret" => Tkn::KeyWord_Ret,
                        _ => Tkn::Name(self.chr_stk.clone()),
                    }
                }
                GenFlag::Str => Tkn::Str(self.chr_stk.clone()),
            };
            Ok(
                LocatedTkn {
                    tkn,
                    pos: chr_counter.clone(),
                    line: line_counter.clone(),
                }
            )
        } else {
            Err(err::ErrKind::SystemErr(err::SystemErr::FlagNotFound))
        }
    }


    fn redict_kind_using_flag(&self) -> Option<GenFlag> {
        Some(match self.last_kind.unwrap_or(CharKind::Other) {
            CharKind::Name => GenFlag::Name,
            CharKind::Num => GenFlag::Number,
            _ => return None,
        })
    }

    fn sort_symbol_tkn(&mut self, chr: &char) -> bool {
        let sym_flag = match self.chr_stk.chars().last() {
            Some(',') => GenFlag::Comma,
            Some('+') => GenFlag::Add,
            Some('*') => GenFlag::Mul,
            Some('-') => GenFlag::Sub,
            Some('/') => GenFlag::Div,
            Some('(') => GenFlag::LParen,
            Some(')') => GenFlag::RParen,
            Some('=') => GenFlag::Equal,
            Some('{') => GenFlag::LBrace,
            Some('}') => GenFlag::RBrace,
            Some(':') => GenFlag::Colon,
            Some('<') => GenFlag::LAngleBracket,
            Some('>') => GenFlag::RAngleBracket,
            _ => return self.get_value_by_flag_ty(&chr, UNSTACKABLE),
        };
        self.over_write_flag::<true, UNSTACKABLE>(sym_flag)
    }

    /// 現在の文字がスタック可能か調べる関数
    /// ## 引数
    /// * curr_kind
    /// 現在の文字の種類
    /// * chr
    /// 現在の文字
    ///
    /// ## 戻り値
    /// スタック可能ならtrueを返す
    /// falseを返すとトークンを生成する処理がされる
    fn check_stkable_chr(
        &mut self,
        curr_kind: &CharKind,
        chr: &char
    ) -> bool {
        if let Some(ref last_kind) = self.last_kind {
            match (last_kind, curr_kind) {
                // 文字と数字は一緒にスタック可能
                (CharKind::Num | CharKind::Name, CharKind::Name | CharKind::Num) =>
                    self.over_write_flag::<false, STACKABLE>(last_kind.flag().unwrap()),
                (_, CharKind::Space) | (CharKind::Space, _) => {
                    if self.gen_flag == Some(GenFlag::Str) {
                        STACKABLE
                    } else { // ここはエラーになる可能性あり
                        if let Some(flag) = self.redict_kind_using_flag() {
                            return self.over_write_flag::<false, UNSTACKABLE>(flag);
                        } else {
                            if last_kind == &CharKind::Op {
                                return self.sort_symbol_tkn(&self.chr_stk.chars().last().unwrap());
                            }
                        }
                        UNSTACKABLE
                    }
                }
                (CharKind::Name | CharKind::Num, k) => {
                    if self.gen_flag == Some(GenFlag::Str) {
                        self.get_value_by_flag_ty(&chr, UNSTACKABLE)
                    } else {
                        if k == &CharKind::Op {
                            self.over_write_flag::<true, UNSTACKABLE>(GenFlag::Name);
                            return UNSTACKABLE;
                        }
                        let flag = match last_kind {
                            CharKind::Name => GenFlag::Name,
                            CharKind::Num => GenFlag::Number,
                            _ => return STACKABLE,
                        };
                        self.over_write_flag::<true, UNSTACKABLE>(flag)
                    }
                }
                (CharKind::Op, r) => {
                    if self.chr_stk.chars().last() == Some('"') {
                        self.chr_stk.pop();
                        if self.gen_flag == Some(GenFlag::Str) {
                            UNSTACKABLE
                        } else {
                            self.over_write_flag::<true, STACKABLE>(GenFlag::Str)
                        }
                    } else {
                        if r == &CharKind::Op {
                            let mut stk = self.chr_stk.clone();
                            stk.push(*chr);
                            let flag = match stk.as_str() {
                                _ => return self.sort_symbol_tkn(&chr),
                            };
                            self.over_write_flag::<true, STACKABLE>(flag)
                        } else {
                            self.sort_symbol_tkn(&chr)
                        }
                    }
                }
                (_, CharKind::Op) => self.get_value_by_flag_ty(&chr, UNSTACKABLE),
                (_, _) => {
                    if self.gen_flag == Some(GenFlag::Str) {
                        STACKABLE
                    } else {
                        UNSTACKABLE
                    }
                }
            }
        } else { // 前回の文字の種類がないので、スタック可能
            STACKABLE
        }
    }

    /// 現在のトークンが文字列のトークンかつ今処理中の文字が'"'なら
    /// 文字のスタックを止める関数
    fn get_value_by_flag_ty(&self, chr: &char, other_flag_value: bool) -> bool {
        if self.gen_flag == Some(GenFlag::Str) {
            if *chr == '"' {
                UNSTACKABLE
            } else {
                STACKABLE
            }
        } else {
            other_flag_value
        }
    }

    /// トークンのフラグを立てる
    /// OW(over write)
    /// がtrueの場合必ず上書きする
    /// OW = "over write"
    /// RV = "return value"
    #[inline(always)]
    fn over_write_flag<const OW: bool, const RV: bool>(&mut self, flag: GenFlag) -> bool {
        if OW { // 上書きモード
            self.gen_flag = Some(flag);
        } else {
            if self.gen_flag.is_none() {
                self.gen_flag = Some(flag);
            }
        }
        RV
    }
}


#[cfg(test)]
mod tests {
    use crate::lex::*;

    fn lexer() -> Lexer {
        Lexer::new()
    }

    #[test]
    fn check_str_tkn() {
        assert_eq!(
            lexer().analy(&"\"hello world!!\" name".to_string()).unwrap(),
            &vec![Tkn::Str("hello world!!".to_string()), Tkn::Name("name".to_string())]
        );
    }
}
