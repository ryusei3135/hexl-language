use crate::err::{self, *};


mod local {
    /// asciiに対応したテーブルを作成する
    pub fn make_char_table() -> [CharKind; 256] {
        let mut table: [CharKind; 256] = [ const { CharKind::Other }; 256 ];

        table[0x09..=0x0D].fill(CharKind::Space);
        table[0x20] = CharKind::Space;
        table[0x21..=0x2F].fill(CharKind::Op);
        table[0x30..=0x39].fill(CharKind::Num);
        table[0x41..=0x5A].fill(CharKind::Name);
        table[0x5F] = CharKind::Name;
        table[0x5B..=0x60].fill(CharKind::Op);
        table[0x61..=0x7A].fill(CharKind::Name);
        table[0x7B..=0x7E].fill(CharKind::Op);
        table
    }


    #[derive(Clone, Debug, PartialEq, Copy)]
    pub enum CharKind {
        Op,
        Num,
        Name,
        Space,
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
        AddrStart,
        AddrEnd,
        Comma,
        Add,
        Mul,
        Number,
        Name,
        Block,
        Other,
        Str,
    }
}

use local::*;


// ===== ここから公開API =====

#[derive(Clone, Debug, PartialEq)]
pub enum Tkn {
    AddrStart,
    AddrEnd,
    Add,
    Mul,
    Comma,
    Block(String),
    Number(String),
    Name(String),
    Str(String),
    Extern,
    Pub,
    Section,
}

impl Tkn {
    pub fn wrap(self) -> Result<Self, err::Err> {
        Ok(self)
    }
}


const STACKABLE: bool = true;
const UNSTACKABLE: bool = false;

pub struct Lexer {
    last_kind: Option<CharKind>,
    gen_flag: Option<GenFlag>,
    chr_stk: String,
    gen_tkns: Vec<Tkn>,
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

    pub fn analy(&mut self, content: &String) -> Result<&Vec<Tkn>, err::Err> {
        const GEN_TKN: bool = false;
        let char_table: [CharKind; 256] = make_char_table();

        for chr in content.chars() {
            let curr_kind = &char_table[chr as usize];
            
            // もし*`last_kind`*がNoneなら、現在の種類同士を比較し、条件をfalseにする
            // もし文字の種類が記号の場合必ず実行する
            if self.last_kind.as_ref().unwrap_or(curr_kind) != curr_kind || curr_kind == &CharKind::Op {
                // スタック可能か調べ不可能ならトークンを生成
                if self.check_stkable_chr(&curr_kind, &chr) == GEN_TKN {
                    if let Ok(tkn) = self.gen_tkn() {
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
                    (CharKind::Op, CharKind::Num) => {
                        self.over_write_flag::<true, STACKABLE>(GenFlag::Number);
                    }
                    (_, _) => {},
                }
            }

            self.chr_stk.push(chr);
            self.last_kind = Some(*curr_kind);
        }

        if self.gen_flag.is_some() {
            self.gen_tkns.push(self.gen_tkn()?);
        }

        Ok(&self.gen_tkns)
    }

    /// トークンを生成しOptionで返す
    fn gen_tkn(&self) -> Result<Tkn, err::Err> {
        if let Some(ref flag) = self.gen_flag {
            match flag {
                GenFlag::AddrStart => Tkn::AddrStart,
                GenFlag::AddrEnd => Tkn::AddrEnd,
                GenFlag::Add => Tkn::Add,
                GenFlag::Mul => Tkn::Mul,
                GenFlag::Block => Tkn::Block(self.chr_stk.clone()),
                GenFlag::Number => Tkn::Number(self.chr_stk.clone()),
                GenFlag::Name => {
                    match self.chr_stk.as_str() {
                        "section" => Tkn::Section,
                        "extern" => Tkn::Extern,
                        "pub" => Tkn::Pub,
                        _ => Tkn::Name(self.chr_stk.clone()),
                    }
                }
                GenFlag::Comma => Tkn::Comma,
                GenFlag::Str => Tkn::Str(self.chr_stk.clone()),
                GenFlag::Other => return Err(Err::SystemErr(SystemErr::FlagNotFound)),
            }.wrap()
        } else {
            Err(err::Err::SystemErr(err::SystemErr::FlagNotFound))
        }
    }

    fn redict_kind_using_flag(&self) -> Option<GenFlag> {
        Some(match self.last_kind.unwrap_or(CharKind::Other) {
            CharKind::Name => GenFlag::Name,
            CharKind::Num => GenFlag::Number,
            _ => return None,
        })
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
                                return match self.chr_stk.chars().last() {
                                    Some('[') => self.over_write_flag::<true, UNSTACKABLE>(GenFlag::AddrStart),
                                    Some(']') => self.over_write_flag::<true, UNSTACKABLE>(GenFlag::AddrEnd),
                                    Some(',') => self.over_write_flag::<true, UNSTACKABLE>(GenFlag::Comma),
                                    Some('+') => self.over_write_flag::<true, UNSTACKABLE>(GenFlag::Add),
                                    Some('*') => self.over_write_flag::<true, UNSTACKABLE>(GenFlag::Mul),
                                    _ => self.get_value_by_flag_ty(&chr, UNSTACKABLE),
                                };
                            }
                        }
                        UNSTACKABLE
                    }
                }
                (CharKind::Name | CharKind::Num, k) => {
                    if chr == &':' {// ブロックを作成
                        self.can_gen_block_flag(&chr)
                    } else {
                        if k == &CharKind::Op {
                            self.get_value_by_flag_ty(&chr, UNSTACKABLE)
                        } else {
                            STACKABLE
                        }
                    }
                }
                (CharKind::Op, _) => {
                    if self.chr_stk.chars().last() == Some('"') {
                        self.chr_stk.pop();
                        if self.gen_flag == Some(GenFlag::Str) {
                            UNSTACKABLE
                        } else {
                            self.over_write_flag::<true, STACKABLE>(GenFlag::Str)
                        }
                    } else {
                        match self.chr_stk.chars().last() {
                            Some('[') => self.over_write_flag::<true, UNSTACKABLE>(GenFlag::AddrStart),
                            Some(']') => self.over_write_flag::<true, UNSTACKABLE>(GenFlag::AddrEnd),
                            Some(',') => self.over_write_flag::<true, UNSTACKABLE>(GenFlag::Comma),
                            Some('+') => self.over_write_flag::<true, UNSTACKABLE>(GenFlag::Add),
                            Some('*') => self.over_write_flag::<true, UNSTACKABLE>(GenFlag::Mul),
                            _ => self.get_value_by_flag_ty(&chr, STACKABLE),
                        }
                    }
                }
                //(CharKind::Name | CharKind::Num, CharKind::Op) => {
                  //  let other_value = self.can_gen_block_flag(&chr); 
                    //self.get_value_by_flag_ty(&chr, other_value)
                //}
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

    /// ブロックのトークンが生成可能か調べる関数
    #[inline(always)]
    fn can_gen_block_flag(&mut self, chr: &char) -> bool {
        if *chr == ':' {
            self.over_write_flag::<true, UNSTACKABLE>(GenFlag::Block)
        } else {
            self.over_write_flag::<false, UNSTACKABLE>(
                self.redict_kind_using_flag()
                    .unwrap_or(GenFlag::Name)
            )
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

    #[test]
    fn check_sections_tkns() {
        assert_eq!(
            lexer().analy(&"section data".to_string()).unwrap(),
            &vec![Tkn::Section, Tkn::Name("data".to_string())]
        );
    }

    #[test]
    fn check_block_tkns() {
        assert_eq!(
            lexer().analy(&"block:".to_string()).unwrap(),
            &vec![Tkn::Block("block".to_string())]
        )
    }
}
