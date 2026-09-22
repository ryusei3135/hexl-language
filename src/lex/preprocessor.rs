use crate::models::*;
use std::collections::HashMap;

/// `#define NAME VALUE` の右辺として実際に登録される値。
/// 今後 `define` 以外のプリプロセッサ命令が増える前提のため、
/// 素の `String` ではなく列挙体として持たせている。
/// (※ `crate::models` に既に同名の型がある場合はそちらに合わせて削除してください)
#[derive(Clone, Debug, PartialEq)]
pub(in crate::lex) 
enum ReplaceVal {
    Str(String),
}

pub(in crate::lex) 
enum PreprocKind {
    Define(ReplaceVal),
}

pub(in crate::lex) 
struct Preprocessor {
    tables: HashMap<String, PreprocKind>,
}

impl Preprocessor {
    pub(super) fn new() -> Self {
        Self {
            tables: HashMap::new(),
        }
    }

    /// 名前がまだ登録されていなければ追加する。
    /// 仕様には書かれていないが、同名の再定義は無視する
    /// (元の `add` の `is_none()` チェックを踏襲)。
    pub(super) fn add(
        &mut self, 
        name: &str, 
        preproc: PreprocKind
    ) {
        if self.tables
            .get(name)
            .is_none() 
        {
            self.tables.insert(
                name.to_string(), 
                preproc
            );
        }
    }

    /// `#NAME` を実際の値に展開する。
    /// 仕様どおり、登録されていない名前が来たら panic する。
    pub(super) fn resolve(
        &self, 
        name: &str
    ) -> &ReplaceVal {
        match self.tables.get(name) {
            Some(PreprocKind::Define(val)) => {
                &val
            }
            None => panic!("undefined preprocessor symbol: `{}`", name),
        }
    }

    /// `#if #NAME` 用: panicせずに「定義済みかどうか」だけを調べる。
    pub(super) fn is_defined(
        &self, 
        name: &str
    ) -> bool {
        self.tables
            .get(name)
            .is_some()
    }
}

/// `#` の直後に来た `Name` トークン(例: "define")を見て、
/// 改行までにスタックした残りの文字列をどう解釈するかを決める。
///
/// 仕様の手順:
/// 1. `#` を見つける
/// 2. 直後に `Name` が来たらフラグを立てる
/// 3. 改行が来るまで文字列をスタック
/// 5. 2で集めた名前を match で条件分岐する (命令が今後増える前提)
/// 6. 3の値と一緒にテーブルに置く
///
/// `name` が未知の命令なら `None` を返す。
pub(in crate::lex) fn sort_preproc<'a>(
    name: &str,
    rest_of_line: &'a str,
) -> Option<(&'a str, PreprocKind)> {
    match name {
        "define" => {
            // `A 10` のような形式を「登録する名前」と「値」に分ける
            let mut parts = rest_of_line
                .trim()
                .splitn(
                    2, 
                    char::is_whitespace
                );
            let target_name = parts.next()?;
            let value = parts
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            Some((
                target_name, 
                PreprocKind::Define(
                    ReplaceVal::Str(
                        value))
            ))
        }
        _ => None,
    }
}
