use std::rc::Rc;

use super::*;
use crate::compiler::{
    err,
    hexl_frontend::lex::{lexer::local::CharKind::Space, preprocessor::Preprocessor},
};

mod local;

use local::*;

#[derive(Debug)]
pub struct Lexer {
    last_kind: Option<CharKind>,
    gen_flag: Option<GenFlag>,
    chr_stk: String,
    pub gen_tkns: Vec<LocatedTkn>,
    preproc_table: Preprocessor,
    pub include_c_file_list: Vec<String>,
    /// `join_sym_tkn`が「直前のトークンと結合してよいか(=本当にソース上で
    /// 隣接した文字だったか)」を判定するために使う。これが無いと、
    /// `gen_tkns`の末尾が`Mul`かどうかだけで判定してしまい、
    /// `byte* = ...`のように`*`と`=`の間に空白があるだけのケースまで
    /// `*=`(MulEq)として誤って結合してしまう。
    saw_gap: bool,
    span: Option<Rc<err::Span>>,
    self_file_path: Rc<String>,
}

impl Lexer {
    /// 実行環境のOS(`#linux` / `#windows` のどちらか)が定義された状態で作る
    pub fn new(self_file_path: &str) -> Self {
        Self::with_preproc(Preprocessor::new().setup_defines(), self_file_path)
    }

    /// `--linux` / `--windows` で選んだOS名(`"linux"` / `"windows"`)のシンボルだけを
    /// 定義して作る。`#if #windows` などの条件がこのOSで切り替わる。
    pub fn with_os(os: &str, self_file_path: &str) -> Self {
        Self::with_preproc(Preprocessor::new().setup_defines_for(os), self_file_path)
    }

    /// これから解析するソースファイルのパスを教える。
    /// 最も外側のファイル(`#include`の先ではないもの)のあるディレクトリが
    /// `ROOT`になり、`#include "dir/file"`は`ROOT/dir/file`として解決される。
    pub fn set_source_file(&mut self, file: Rc<String>) {
        if (*file).ends_with(".c") || (*file).ends_with(".h") {
            self.include_c_file_list.push(file.to_string());
        } else {
            crate::compiler::hexl_frontend::include_root::set_root_from_file(std::path::Path::new(
                (*file).as_str(),
            ));
        }
    }

    fn with_preproc(preproc_table: Preprocessor, file_path: &str) -> Self {
        Self {
            last_kind: None,
            gen_flag: None,
            chr_stk: String::new(),
            gen_tkns: Vec::new(),
            preproc_table,
            include_c_file_list: Vec::new(),
            saw_gap: true,
            span: None,
            self_file_path: Rc::new(file_path.to_string()),
        }
    }

    pub fn analy(&mut self, content: &str) -> Result<(), err::lex_err::LexErrs> {
        self.gen_tkns = Vec::new();
        self.chr_stk = String::new();
        self.last_kind = None;
        self.gen_flag = None;
        self.saw_gap = true;
        let char_table: [CharKind; 256] = make_char_table();

        // `#define ...` を登録し、`#NAME` をその値に展開したソースを作る。
        let content = self
            .preproc_table
            .preprocess(content)
            .unwrap_or(content.to_string());
        // `#include "file.c"` / `#include "file.h"` は展開せず、名前だけ保存する
        for file in self.preproc_table.take_c_files() {
            if !self.include_c_file_list.contains(&file) {
                self.include_c_file_list.push(file);
            }
        }

        // 現在の行の何文字目か
        let mut chr_counter: usize = 1;
        // 現在の行
        let mut line_counter: usize = 1;

        let chars: Vec<char> = content.chars().collect();
        let lines: Vec<&str> = content.split('\n').collect();

        self.span = Some(Rc::new(err::Span::new(
            line_counter,
            chr_counter,
            lines[line_counter - 1].to_string(),
            &self.self_file_path,
        )));

        for (idx, &chr) in chars.iter().enumerate() {
            let curr_kind = &char_table[chr as usize];

            // `3.14` の `.`: 整数部に小数点を積み、以降の数字を同じ数値トークンに続ける。
            // (`1..2` や `a.b` は、次の文字が数字ではないのでここには入らない)
            if chr == '.' && self.continues_as_float(chars.get(idx + 1).copied()) {
                self.chr_stk.push(chr);
                self.last_kind = Some(CharKind::Num);
                self.gen_flag = Some(GenFlag::Number);
                chr_counter += 1;
                continue;
            }

            // もし*`last_kind`*がNoneなら、現在の種類同士を比較し、条件をfalseにする
            // もし文字の種類が記号の場合必ず実行する
            let mut consumed = false;
            if self.last_kind.as_ref().unwrap_or(curr_kind) != curr_kind
                || curr_kind == &CharKind::Op
            {
                // スタック可能か調べ、不可能ならトークンを生成
                let stk_result = self.check_stkable_chr(curr_kind, &chr);
                if stk_result != StkResult::Stackable {
                    match self.gen_tkn() {
                        Ok(tkn) => {
                            let adjacent = !self.saw_gap;
                            let t = LocatedTkn {
                                tkn: self.join_sym_tkn(&tkn, adjacent).unwrap_or(tkn.tkn.clone()),
                                span: Rc::clone(self.span.as_ref().unwrap()),
                            };
                            self.gen_tkns.push(t);
                            self.saw_gap = false;
                        }
                        Err(_e) => {}
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
                        (_, _) => {}
                    }
                }
                // 改行が来たので、lineをインクリメント
                if curr_kind == &CharKind::Ln {
                    // 文字の場所を初期化
                    chr_counter = 0;
                    line_counter += 1;
                    self.span = Some(Rc::new(err::Span::new(
                        line_counter,
                        chr_counter,
                        lines[line_counter - 1].to_string(),
                        &self.self_file_path,
                    )));
                }

                self.chr_stk.push(chr);
                self.last_kind = Some(*curr_kind);
            }

            // 空白・改行を見たら、以降トークンを積むまでは「隙間あり」として
            // 記録する。この判定は、上のトークン生成処理より後に行うことで、
            // 「この空白/改行の直前で確定したトークン」自体の隣接判定には
            // 影響しないようにしている。
            if matches!(curr_kind, CharKind::Space | CharKind::Ln) {
                self.saw_gap = true;
            }

            chr_counter += 1;
        }
        self.check_stkable_chr(&CharKind::Other, &'\0');

        if self.gen_flag.is_some() {
            self.gen_tkns.push(self.gen_tkn()?);
        }

        Ok(())
    }

    /// トークンを生成しOptionで返す
    fn gen_tkn(&self) -> Result<LocatedTkn, err::lex_err::LexErrs> {
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
                GenFlag::Surplus => Tkn::Surplus,
                GenFlag::Inversion => Tkn::Inversion,
                GenFlag::And => Tkn::And,
                GenFlag::Xor => Tkn::Xor,
                GenFlag::Or => Tkn::Or,
                GenFlag::Dot => Tkn::Dot,
                GenFlag::LAngleBracket => Tkn::LAngleBracket,
                GenFlag::RAngleBracket => Tkn::RAngleBracket,
                GenFlag::LBracket => Tkn::LBracket,
                GenFlag::RBracket => Tkn::RBracket,
                GenFlag::Question => Tkn::Question,

                GenFlag::CompleSyn => Tkn::CompleSyn,
                GenFlag::Number => {
                    if self.chr_stk.starts_with("0x") {
                        // 元のコードは .unwrap() で不正な16進数（例: "0x" だけ、
                        // "0xZZ" など）のときにパニックしていた。
                        // gen_tkn は Result を返すので、ここはちゃんと
                        // エラーとして伝播させる。
                        let Ok(num) =
                            u32::from_str_radix(self.chr_stk.trim_start_matches("0x"), 16)
                        else {
                            return crate::lex_err!(
                                Rc::clone(self.span.as_ref().unwrap()),
                                NumIsInvalid
                            );
                        };
                        Tkn::Number(num.to_string())
                    } else if self.chr_stk.contains('.') {
                        // `3.14x` のように数字以外が混ざったものは不正な数値
                        if self.chr_stk.parse::<f64>().is_err() {
                            return crate::lex_err!(
                                Rc::clone(self.span.as_ref().unwrap()),
                                NumIsInvalid
                            );
                        }
                        Tkn::Float(self.chr_stk.clone())
                    } else {
                        Tkn::Number(self.chr_stk.clone())
                    }
                }
                GenFlag::Not => Tkn::Not,
                GenFlag::Name => match self.chr_stk.as_str() {
                    "ret" => Tkn::KeyWordRet,
                    "cond" => Tkn::KeyWordCond,
                    "loop" => Tkn::KeyWordLoop,
                    "pub" => Tkn::KeyWordPub,
                    "struct" => Tkn::KeyWordStruct,
                    "variant" => Tkn::KeyWordVariant,
                    "const" => Tkn::KeyWordConst,
                    "static" => Tkn::KeyWordStatic,
                    "mut" => Tkn::KeyWordMut,
                    "Self" => Tkn::KeyWordSelf,
                    "must" => Tkn::KeyWordMust,
                    "of" => Tkn::KeyWordOf,
                    "continue" => Tkn::KeyWordContinue,
                    "break" => Tkn::KeyWordBreak,
                    "where" => Tkn::KeyWordWhere,
                    "allow" => Tkn::KeyWordAllow,
                    "else" => Tkn::KeyWordElse,
                    "impl" => Tkn::KeyWordImpl,
                    "extern" => Tkn::KeyWordExtern,
                    "register" => Tkn::KeyWordRegister,
                    "async" => Tkn::KeyWordAsync,
                    "await" => Tkn::KeyWordAwait,
                    "move" => Tkn::KeyWordMove,
                    "allow" => Tkn::KeyWordAllow,
                    "type" => Tkn::KeyWordType,
                    "sizeof" => Tkn::KeyWordSizeof,
                    _ => Tkn::Name(self.chr_stk.clone()),
                },
                GenFlag::Str => Tkn::Str(self.chr_stk.clone()),
            };
            Ok(LocatedTkn {
                tkn,
                span: Rc::clone(self.span.as_ref().unwrap()),
            })
        } else {
            crate::lex_err!(Rc::clone(self.span.as_ref().unwrap()), FlagNotFound)
        }
    }

    /// 今積んでいる数字の列に、次の `.` を小数点として続けてよいか調べる。
    /// - `next`: `.` の次の文字
    ///
    /// 次の全てを満たすときだけ小数点になる:
    /// - 積んでいるのが10進の整数だけ (`0x1F` / 名前 / すでに `.` を含む数値は除く)
    /// - `.` の次が数字 (`1..2` や `3.foo` は小数にしない)
    /// - 直前のトークンが `.` ではない (`a.0.1` は2つのメンバーアクセスとして扱う)
    fn continues_as_float(&self, next: Option<char>) -> bool {
        self.last_kind == Some(CharKind::Num)
            && matches!(self.gen_flag, None | Some(GenFlag::Number))
            && !self.chr_stk.is_empty()
            && self.chr_stk.chars().all(|c| c.is_ascii_digit())
            && next.map_or(false, |c| c.is_ascii_digit())
            && !matches!(self.gen_tkns.last().map(|t| &t.tkn), Some(Tkn::Dot))
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
            Some('&') => GenFlag::And,
            Some('^') => GenFlag::Xor,
            Some('|') => GenFlag::Or,
            Some('.') => GenFlag::Dot,
            Some('!') => GenFlag::Not,
            Some('%') => GenFlag::Surplus,
            Some('~') => GenFlag::Inversion,
            Some('?') => GenFlag::Question,
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
    fn check_stkable_chr(&mut self, curr_kind: &CharKind, chr: &char) -> StkResult {
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
                    } else {
                        // ここはエラーになる可能性あり
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
                        StkResult::Consumed
                    } else if self.gen_flag == Some(GenFlag::Str) {
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
        } else {
            // 前回の文字の種類がないので、スタック可能
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
        if OW {
            // 上書きモード
            self.gen_flag = Some(flag);
        } else {
            if self.gen_flag.is_none() {
                self.gen_flag = Some(flag);
            }
        }
    }
}

#[cfg(test)]
mod tests;
