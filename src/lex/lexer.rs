use crate::err;
use super::*;


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
        Or,
        Dot,
        LAngleBracket,
        RAngleBracket,
        LBracket,
        RBracket,
        Not,

        CompleSyn,

        Number,
        Name,
        Str,
    }

    /// `check_stkable_chr` の戻り値。
    ///
    /// 元のコードは `bool`(STACKABLE/UNSTACKABLE) だけだったため、
    /// 「現在の文字をすでに消費済みなので `chr_stk` に積んではいけない」
    /// というケース（文字列の閉じクォートなど）を表現できなかった。
    /// これが「文字列トークンの後ろが壊れる」バグの直接原因だったため、
    /// 第三の状態 `Consumed` を追加して区別できるようにする。
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
}

use local::*;



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

    pub fn analy(&mut self, content: &String) -> Result<(), err::ErrKind> {
        self.gen_tkns = Vec::new();
        self.chr_stk = String::new();
        self.last_kind = None;
        self.gen_flag = None;
        let char_table: [CharKind; 256] = make_char_table();

        // 現在の行の何文字目か
        let mut chr_counter: usize = 1;
        // 現在の行
        let mut line_counter: usize = 1;

        for chr in content.chars() {
            let curr_kind = &char_table[chr as usize];

            // もし*`last_kind`*がNoneなら、現在の種類同士を比較し、条件をfalseにする
            // もし文字の種類が記号の場合必ず実行する
            let mut consumed = false;
            if self.last_kind.as_ref().unwrap_or(curr_kind) != curr_kind || curr_kind == &CharKind::Op {
                // スタック可能か調べ、不可能ならトークンを生成
                let stk_result = self.check_stkable_chr(curr_kind, &chr);
                if stk_result != StkResult::Stackable {
                    match self.gen_tkn(&line_counter, &chr_counter) {
                        Ok(tkn) => {
                            let t = LocatedTkn {
                                tkn: self.join_sym_tkn(&tkn).unwrap_or(tkn.tkn.clone()),
                                pos: chr_counter.clone(),
                                line: line_counter.clone(),
                            };
                            self.gen_tkns.push(t);
                        }
                        Err(_e) => {
                        }
                    }
                    self.chr_stk.clear();
                    self.gen_flag = None;
                }
                // 現在の文字（例: 文字列の閉じクォート）はすでに消費済みなので、
                // chr_stk には積まない。
                if stk_result == StkResult::Consumed {
                    consumed = true;
                    self.last_kind = None;
                }
            }

            if !consumed {
                if self.gen_flag.is_none() {
                    match (self.last_kind.unwrap_or(CharKind::Other), curr_kind) {
                        (CharKind::Op | CharKind::Space, CharKind::Name) => {
                            self.over_write_flag::<true>(GenFlag::Name);
                        }
                        (CharKind::Op | CharKind::Other | CharKind::Space, CharKind::Num) => {
                            self.over_write_flag::<true>(GenFlag::Number);
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
            }
            chr_counter += 1;
        }
        self.check_stkable_chr(&CharKind::Other, &'\0');

        if self.gen_flag.is_some() {
            self.gen_tkns.push(
                self.gen_tkn(&line_counter, &chr_counter)?
            );
        }

        Ok(())
    }

    /// トークンを生成しOptionで返す
    fn gen_tkn(
        &self,
        line_counter: &usize,
        chr_counter: &usize
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
                GenFlag::Or => Tkn::Or,
                GenFlag::Dot => Tkn::Dot,
                GenFlag::LAngleBracket => Tkn::LAngleBracket,
                GenFlag::RAngleBracket => Tkn::RAngleBracket,
                GenFlag::LBracket => Tkn::LBracket,
                GenFlag::RBracket => Tkn::RBracket,

                GenFlag::CompleSyn => Tkn::CompleSyn,
                GenFlag::Number => {
                    if self.chr_stk.starts_with("0x") {
                        // 元のコードは .unwrap() で不正な16進数（例: "0x" だけ、
                        // "0xZZ" など）のときにパニックしていた。
                        // gen_tkn は Result を返すので、ここはちゃんと
                        // エラーとして伝播させる。
                        let num = u32::from_str_radix(
                            self.chr_stk.trim_start_matches("0x"),
                            16,
                        ).map_err(|_| {
                            err::LexErr::ThisNumIsInvalid
                                .fmt(&line_counter, &chr_counter)
                        })?;
                        Tkn::Number(num.to_string())
                    } else {
                        Tkn::Number(self.chr_stk.clone())
                    }
                }
                GenFlag::Not => Tkn::Not,
                GenFlag::Name => {
                    match self.chr_stk.as_str() {
                        "ret" => Tkn::KeyWordRet,
                        "cond" => Tkn::KeyWordCond,
                        "loop" => Tkn::KeyWordLoop,
                        "pub" => Tkn::KeyWordPub,
                        "struct" => Tkn::KeyWordStruct,
                        "enum" => Tkn::KeyWordEnum,
                        "const" => Tkn::KeyWordConst,
                        "Self" => Tkn::KeyWordSelf,
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

    fn sort_symbol_tkn(&mut self, chr: &char) -> StkResult {
        let sym_flag = match self.chr_stk.chars().last() {
            Some('#') => GenFlag::CompleSyn,
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
            Some('[') => GenFlag::LBracket,
            Some(']') => GenFlag::RBracket,
            Some('|') => GenFlag::Or,
            Some('.') => GenFlag::Dot,
            Some('!') => GenFlag::Not,
            _ => return self.get_value_by_flag_ty(chr, StkResult::GenTkn),
        };
        self.over_write_flag::<true>(sym_flag);
        StkResult::GenTkn
    }

    /// 現在の文字がスタック可能か調べる関数
    /// ## 引数
    /// * curr_kind
    /// 現在の文字の種類
    /// * chr
    /// 現在の文字
    ///
    /// ## 戻り値
    /// `StkResult::Stackable`  -> 現在の文字をスタックに積んでよい
    /// `StkResult::GenTkn`     -> トークンを生成し、現在の文字は通常通り積む
    /// `StkResult::Consumed`   -> トークンを生成し、現在の文字は消費済み（積まない）
    fn check_stkable_chr(
        &mut self,
        curr_kind: &CharKind,
        chr: &char
    ) -> StkResult {
        if let Some(ref last_kind) = self.last_kind {
            match (last_kind, curr_kind) {
                // 文字と数字は一緒にスタック可能
                (CharKind::Num | CharKind::Name, CharKind::Name | CharKind::Num) => {
                    // 元コードは over_write_flag::<false, STACKABLE> だったが、
                    // STACKABLE は定数なので RV は常に Stackable で固定してよい。
                    if self.gen_flag.is_none() {
                        self.gen_flag = last_kind.flag();
                    }
                    StkResult::Stackable
                }
                (_, CharKind::Space) | (CharKind::Space, _) => {
                    if self.gen_flag == Some(GenFlag::Str) {
                        StkResult::Stackable
                    } else { // ここはエラーになる可能性あり
                        if let Some(flag) = self.redict_kind_using_flag() {
                            if self.gen_flag.is_none() {
                                self.gen_flag = Some(flag);
                            }
                            return StkResult::GenTkn;
                        } else {
                            // 元のコードは `.unwrap()` していたため、
                            // chr_stk が空のとき（直前の文字を Consumed で
                            // 消費済みのときなど）にパニックする欠陥があった。
                            // 空なら単にトークン生成のみ行う。
                            if last_kind == &CharKind::Op {
                                if let Some(last_chr) = self.chr_stk.chars().last() {
                                    return self.sort_symbol_tkn(&last_chr);
                                }
                            }
                        }
                        StkResult::GenTkn
                    }
                }
                (CharKind::Name | CharKind::Num, _) => {
                    if self.gen_flag == Some(GenFlag::Str) {
                        self.get_value_by_flag_ty(chr, StkResult::GenTkn)
                    } else {
                        // 元のコードはここで無条件に gen_flag を
                        // last_kind 由来の値で上書きしていたため、
                        // 0x1F のように Num で始まり Name 種別の文字
                        // (a-f, x) を含む16進数リテラルが、末尾の文字種
                        // だけで判定されて Number ではなく Name に
                        // 化けてしまう不具合があった。
                        // すでに gen_flag が確定している場合はそれを
                        // 優先し、未確定の場合のみ last_kind から補う。
                        if self.gen_flag.is_none() {
                            let flag = match last_kind {
                                CharKind::Name => GenFlag::Name,
                                CharKind::Num => GenFlag::Number,
                                _ => return StkResult::Stackable,
                            };
                            self.gen_flag = Some(flag);
                        }
                        StkResult::GenTkn
                    }
                }
                (CharKind::Op, r) => {
                    if self.chr_stk.chars().last() == Some('"') {
                        // 直前にスタックされた文字が開きクォートだったケース。
                        // 開きクォートを取り除いて文字列モードへ入る。
                        self.chr_stk.pop();
                        if self.gen_flag == Some(GenFlag::Str) {
                            StkResult::GenTkn
                        } else {
                            self.gen_flag = Some(GenFlag::Str);
                            StkResult::Stackable
                        }
                    } else if self.gen_flag == Some(GenFlag::Str) && *chr == '"' {
                        // ===== 文字列直後トークン消失バグの本体 =====
                        // 文字列モード中に閉じクォート `"` が来たケース。
                        // 元のコードはここで GenTkn 相当の bool (UNSTACKABLE) を
                        // 返すだけだったため、トークン生成後に呼び出し元の
                        // analy() が無条件に `chr_stk.push(chr)` を実行し、
                        // 消費したはずの閉じクォートが再度 chr_stk に
                        // 積まれてしまっていた。その結果、文字列トークンの
                        // 直後の数文字ぶん gen_tkn が FlagNotFound で
                        // 失敗し続け、トークンが欠落する原因になっていた。
                        //
                        // Consumed を返すことで、analy() 側に
                        // 「この文字は積むな」と明示的に伝える。
                        StkResult::Consumed
                    } else if self.gen_flag == Some(GenFlag::Str) {
                        // ===== 文字列内の演算子記号バグ =====
                        // 文字列モード中で、かつ今の文字が閉じクォートでも
                        // ないなら（例: "a+b" の '+'）、それは単なる文字列の
                        // 中身であり、記号として解釈してはいけない。
                        // 元のコードはこのケースをチェックせず else に
                        // 落としていたため、文字列内の演算子記号が
                        // sort_symbol_tkn に渡ってトークンが分断され、
                        // 文字列の中身が壊れていた。
                        StkResult::Stackable
                    } else {
                        if r == &CharKind::Op {
                            // 元のコードは2文字演算子（==, <=, >= など）を
                            // 先読みしようとして `stk` を組み立てていたが、
                            // match の腕が `_ => return ...` だけだったため
                            // 実質的にデッドコードで、常に1文字ずつ
                            // sort_symbol_tkn に流れていた。
                            // 複数文字演算子は未実装のため、ここでは
                            // 単純に1文字ずつ処理する元の実質動作を維持する。
                            self.sort_symbol_tkn(chr)
                        } else {
                            self.sort_symbol_tkn(chr)
                        }
                    }
                }
                (_, CharKind::Op) => self.get_value_by_flag_ty(chr, StkResult::GenTkn),
                (_, _) => {
                    if self.gen_flag == Some(GenFlag::Str) {
                        StkResult::Stackable
                    } else {
                        StkResult::GenTkn
                    }
                }
            }
        } else { // 前回の文字の種類がないので、スタック可能
            StkResult::Stackable
        }
    }

    /// 現在のトークンが文字列のトークンかつ今処理中の文字が'"'なら
    /// 文字のスタックを止める関数
    fn get_value_by_flag_ty(&self, chr: &char, other_flag_value: StkResult) -> StkResult {
        if self.gen_flag == Some(GenFlag::Str) {
            if *chr == '"' {
                StkResult::Consumed
            } else {
                StkResult::Stackable
            }
        } else {
            other_flag_value
        }
    }

    /// トークンのフラグを立てる
    /// OW(over write)
    /// がtrueの場合必ず上書きする
    /// OW = "over write"
    #[inline(always)]
    fn over_write_flag<const OW: bool>(&mut self, flag: GenFlag) {
        if OW { // 上書きモード
            self.gen_flag = Some(flag);
        } else {
            if self.gen_flag.is_none() {
                self.gen_flag = Some(flag);
            }
        }
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
        let mut lex = lexer();
        lex.analy(&"\"hello world!!\" name".to_string()).unwrap();
        let tkns: Vec<Tkn> = lex.gen_tkns.into_iter().map(|t| t.tkn).collect();
        assert_eq!(
            tkns,
            vec![Tkn::Str("hello world!!".to_string()), Tkn::Name("name".to_string())]
        );
    }

    #[test]
    fn check_str_followed_by_symbol() {
        // 文字列の直後に space を挟まず記号が来るケース。
        // 修正前はここで後続が消える/壊れるバグがあった。
        let mut lex = lexer();
        lex.analy(&"\"abc\"+1".to_string()).unwrap();
        let tkns: Vec<Tkn> = lex.gen_tkns.into_iter().map(|t| t.tkn).collect();
        assert_eq!(
            tkns,
            vec![
                Tkn::Str("abc".to_string()),
                Tkn::Add,
                Tkn::Number("1".to_string()),
            ]
        );
    }

    #[test]
    fn check_hex_number() {
        let mut lex = lexer();
        lex.analy(&"0x1F".to_string()).unwrap();
        let tkns: Vec<Tkn> = lex.gen_tkns.into_iter().map(|t| t.tkn).collect();
        assert_eq!(tkns, vec![Tkn::Number("31".to_string())]);
    }

    #[test]
    fn check_invalid_hex_number_errors_instead_of_panicking() {
        let mut lex = lexer();
        let result = lex.analy(&"0xZZ".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn check_two_strings_back_to_back() {
        let mut lex = lexer();
        lex.analy(&"\"abc\"\"def\"".to_string()).unwrap();
        let tkns: Vec<Tkn> = lex.gen_tkns.into_iter().map(|t| t.tkn).collect();
        assert_eq!(
            tkns,
            vec![Tkn::Str("abc".to_string()), Tkn::Str("def".to_string())]
        );
    }

    #[test]
    fn check_empty_string_literal() {
        let mut lex = lexer();
        lex.analy(&"\"\"".to_string()).unwrap();
        let tkns: Vec<Tkn> = lex.gen_tkns.into_iter().map(|t| t.tkn).collect();
        assert_eq!(tkns, vec![Tkn::Str("".to_string())]);
    }

    #[test]
    fn check_operators_inside_string_are_not_split() {
        // 文字列リテラル内の演算子記号がトークンとして分断されないこと。
        // 修正前は "a+b-c" が Add, Name("b"), Sub, Name("c"), Str("") に
        // 壊れていた。
        let mut lex = lexer();
        lex.analy(&"\"a+b-c\"".to_string()).unwrap();
        let tkns: Vec<Tkn> = lex.gen_tkns.into_iter().map(|t| t.tkn).collect();
        assert_eq!(tkns, vec![Tkn::Str("a+b-c".to_string())]);
    }

    #[test]
    fn check_string_in_middle_of_expression() {
        let mut lex = lexer();
        lex.analy(&"1+\"mid\"+2".to_string()).unwrap();
        let tkns: Vec<Tkn> = lex.gen_tkns.into_iter().map(|t| t.tkn).collect();
        assert_eq!(
            tkns,
            vec![
                Tkn::Number("1".to_string()),
                Tkn::Add,
                Tkn::Str("mid".to_string()),
                Tkn::Add,
                Tkn::Number("2".to_string()),
            ]
        );
    }

    #[test]
    fn check_name_immediately_followed_by_string() {
        let mut lex = lexer();
        lex.analy(&"name\"abc\"".to_string()).unwrap();
        let tkns: Vec<Tkn> = lex.gen_tkns.into_iter().map(|t| t.tkn).collect();
        assert_eq!(
            tkns,
            vec![Tkn::Name("name".to_string()), Tkn::Str("abc".to_string())]
        );
    }

    #[test]
    fn check_basic_expression_unaffected() {
        let mut lex = lexer();
        lex.analy(&"x = 1 + 2".to_string()).unwrap();
        let tkns: Vec<Tkn> = lex.gen_tkns.into_iter().map(|t| t.tkn).collect();
        assert_eq!(
            tkns,
            vec![
                Tkn::Name("x".to_string()),
                Tkn::Equal,
                Tkn::Number("1".to_string()),
                Tkn::Add,
                Tkn::Number("2".to_string()),
            ]
        );
    }

    #[test]
    fn check_not_eq_tkn() {
        // `!=` が `Not, Equal` の2トークンに分かれず、
        // `NotEq` 1トークンとして生成されることを確認する。
        let mut lex = lexer();
        lex.analy(&"a != b".to_string()).unwrap();
        let tkns: Vec<Tkn> = lex.gen_tkns.into_iter().map(|t| t.tkn).collect();
        assert_eq!(
            tkns,
            vec![
                Tkn::Name("a".to_string()),
                Tkn::NotEq,
                Tkn::Name("b".to_string()),
            ]
        );
    }

    #[test]
    fn check_ret_keyword() {
        let mut lex = lexer();
        lex.analy(&"ret 5".to_string()).unwrap();
        let tkns: Vec<Tkn> = lex.gen_tkns.into_iter().map(|t| t.tkn).collect();
        assert_eq!(tkns, vec![Tkn::KeyWordRet, Tkn::Number("5".to_string())]);
    }

    #[test]
    fn check_self_keyword() {
        let mut lex = lexer();
        lex.analy(&"Self".to_string()).unwrap();
        let tkns: Vec<Tkn> = lex.gen_tkns.into_iter().map(|t| t.tkn).collect();
        assert_eq!(tkns, vec![Tkn::KeyWordSelf]);
    }

    #[test]
    fn check_bracket_tkns() {
        let mut lex = lexer();
        lex.analy(&"[ty 4]".to_string()).unwrap();
        let tkns: Vec<Tkn> = lex.gen_tkns.into_iter().map(|t| t.tkn).collect();
        assert_eq!(
            tkns,
            vec![
                Tkn::LBracket,
                Tkn::Name("ty".to_string()),
                Tkn::Number("4".to_string()),
                Tkn::RBracket,
            ]
        );
    }

    #[test]
    fn check_call_with_args() {
        let mut lex = lexer();
        lex.analy(&"f(1,2)".to_string()).unwrap();
        let tkns: Vec<Tkn> = lex.gen_tkns.into_iter().map(|t| t.tkn).collect();
        assert_eq!(
            tkns,
            vec![
                Tkn::Name("f".to_string()),
                Tkn::LParen,
                Tkn::Number("1".to_string()),
                Tkn::Comma,
                Tkn::Number("2".to_string()),
                Tkn::RParen,
            ]
        );
    }
}
