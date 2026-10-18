//! トークン列 -> CAST。typedef名の判定のため、宣言を読みながら表を更新する。
//!
//! `Parser`の本体(状態とエントリポイント)だけをここに置き、構文ごとに`impl Parser`を分割している。
//!
//! | ファイル | 内容 |
//! |---|---|
//! | `function.rs`    | 関数定義・仮引数・`return`・関数本体(`{ }`) |
//! | `iteration.rs`   | `while` / `do-while` / `for` / `break` / `continue` |
//! | `conditional.rs` | `if` / `switch` / `case` / `default` / 三項演算子 |
//! | `data_type.rs`   | 型指定子・struct/union/enum・宣言子・typedef・初期化子・変数宣言 |
//! | `other.rs`       | トークン操作・文の振り分け・式・定数式の評価・リテラルの展開 |

mod conditional;
mod data_type;
mod function;
mod iteration;
mod other;

use super::cast::*;
use super::error::*;
use super::prelude::{Token, TokenInfo};
use std::collections::{HashMap, HashSet};

pub struct Parser {
    toks: Vec<TokenInfo>,
    pos: usize,
    typedefs: HashMap<String, CType>,
    /// 列挙定数名 -> (列挙型名, メンバー名, 値)
    enum_consts: HashMap<String, (String, String, i64)>,
    items: Vec<CItem>,
    defined_tags: HashSet<String>,
    anon: usize,
    pub warnings: Vec<String>,
}

impl Parser {
    pub fn new(mut toks: Vec<TokenInfo>) -> Self {
        if !matches!(toks.last().map(|t| &t.token), Some(Token::Eof)) {
            let (line, col) = toks.last().map(|t| (t.line, t.col)).unwrap_or((1, 1));
            toks.push(TokenInfo {
                token: Token::Eof,
                line,
                col,
                start: 0,
                end: 0,
                bol: true,
                space_before: true,
            });
        }
        Parser {
            toks,
            pos: 0,
            typedefs: HashMap::new(),
            enum_consts: HashMap::new(),
            items: Vec::new(),
            defined_tags: HashSet::new(),
            anon: 0,
            warnings: Vec::new(),
        }
    }

    pub fn parse_unit(mut self) -> PResult<(Vec<CItem>, Vec<String>)> {
        while !matches!(self.tok(), Token::Eof) {
            self.parse_external()?;
        }
        Ok((self.items, self.warnings))
    }
}
