//! `#include`の管理リスト。
//!
//! `#include`に関する「もう処理した」の記録を、1つのリスト(`IncludeList`)に集める。
//! 以前は、段ごとに別々の`HashSet`や`Vec`で持っていた
//! (プリプロセッサの`collected_files` / `c_files`の重複判定、パーサーの
//! `included_visited` / `included_type_visited`)。
//!
//! 同じ(段, キー)がもう載っていれば、2回目以降は**無視**する。
//! `insert`が`false`を返したら、呼び出し側は何もせず次へ進む。
//!
//! ## 段(`IncludeStage`)
//! 同じファイルでも、段が違えば別々に処理する必要がある
//! (`#define`を集めたファイルも、型を取り込むときにもう一度読む)。
//! そのため、キーに段を含めて1つのリストで区別する。
//!
//! | 段 | キー | 持ち主 |
//! |---|---|---|
//! | `CFile` | `#include "a.c"`の書かれたとおりの名前 | `Preprocessor` |
//! | `Define` | `#define`を集めたファイルのパス | `Preprocessor` |
//! | `Generic` | `ファイル|モジュール名`(ジェネリクスを集め済み) | `Parser` |
//! | `Type` | 公開型を取り込み済みのファイルのパス | `Parser` |
//! | `Include` | `#include`ノードの識別子(`include_key`) | `Parser` |
//!
//! リストは持ち主ごとに1つ(`Preprocessor`は1つ、`Parser`は1つ)。
//! `Include`段は、**ファイルごと**に独立している。`a.hexl`と`b.hexl`が
//! どちらも`mem.hexl`を`#include`していても、それぞれのファイルが
//! 自分の`#include`として`mem.hexl`の関数を使えなければならないため。

use crate::compiler::ast::{ImportKind, ModPath};
use crate::compiler::hexl_frontend::include_root;

/// 何のために処理したかの種類
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IncludeStage {
    /// `#include "file.c"` / `"file.h"`の名前の保存
    CFile,
    /// `#include`先の`#define`の収集
    Define,
    /// `#include`先のジェネリクスな関数/型の収集
    Generic,
    /// `#include`先の公開型ノードの取り込み
    Type,
    /// `#include`ノードそのもの(後段のIRへ渡す)
    Include,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct IncludeEntry {
    stage: IncludeStage,
    key: String,
}

/// 処理済みの`#include`を、登録した順に1つのリストで持つ
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IncludeList {
    entries: Vec<IncludeEntry>,
}

impl IncludeList {
    pub fn new() -> Self {
        Self::default()
    }

    /// (`stage`, `key`)がもう載っているか
    pub fn contains(&self, stage: IncludeStage, key: &str) -> bool {
        self.entries
            .iter()
            .any(|e| e.stage == stage && e.key == key)
    }

    /// (`stage`, `key`)をリストへ載せる。
    /// 初めてなら`true`。もう載っていれば何もせず`false`(= 重複なので無視する)。
    pub fn insert(&mut self, stage: IncludeStage, key: impl Into<String>) -> bool {
        let key = key.into();
        if self.contains(stage, &key) {
            return false;
        }
        self.entries.push(IncludeEntry { stage, key });
        true
    }

    /// `stage`のものだけを写した新しいリスト。
    /// 別のファイルを処理する子の`Parser`へ、必要な段だけを引き継ぐのに使う。
    pub fn only(&self, stage: IncludeStage) -> Self {
        Self {
            entries: self
                .entries
                .iter()
                .filter(|e| e.stage == stage)
                .cloned()
                .collect(),
        }
    }

    /// `other`の`stage`のものを、重複を除いて自分へ足す。
    pub fn merge(&mut self, other: &IncludeList, stage: IncludeStage) {
        for e in other.entries.iter().filter(|e| e.stage == stage) {
            self.insert(stage, e.key.clone());
        }
    }

    /// 載っている数(全ての段の合計)
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// `stage`のキーを、登録した順に返す
    pub fn keys(&self, stage: IncludeStage) -> impl Iterator<Item = &str> {
        self.entries
            .iter()
            .filter(move |e| e.stage == stage)
            .map(|e| e.key.as_str())
    }
}

/// `#include`が、同じ結果を作るかどうかを決めるキー。
///
/// 次が全て同じなら、同じ`#include`(重複)とみなす。
/// - 解決後のファイルのパス(`ROOT`基準 / `std`は標準ライブラリ)
/// - 取り込み方
///   - モジュール: モジュール名(`Name=`の別名、無ければファイル名)
///   - `::func`: 関数名
///   - `::*`
///   - 従来の`mod::file`
///
/// 同じファイルでも、別名が違えば関数名が変わる(`a::f`と`b::f`)ので、
/// 別の`#include`として両方残す。
pub fn include_key(path: &ModPath) -> String {
    let full = include_root::resolve_for(path, &path.gen_path());
    let how = match &path.kind {
        ImportKind::Auto => "auto".to_string(),
        ImportKind::Module(alias) => {
            let name = alias.clone().unwrap_or_else(|| {
                std::path::Path::new(&path.gen_path())
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or_default()
                    .to_string()
            });
            format!("mod:{name}")
        }
        ImportKind::Func(name) => format!("fn:{name}"),
        ImportKind::Glob => "glob".to_string(),
    };
    format!("{full}|{how}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_ignores_duplicates() {
        let mut l = IncludeList::new();
        assert!(l.insert(IncludeStage::Define, "a.hexl"));
        assert!(!l.insert(IncludeStage::Define, "a.hexl"));
        assert_eq!(l.len(), 1);
    }

    #[test]
    fn same_key_in_different_stage_is_not_duplicate() {
        let mut l = IncludeList::new();
        assert!(l.insert(IncludeStage::Define, "a.hexl"));
        assert!(l.insert(IncludeStage::Type, "a.hexl"));
        assert!(!l.insert(IncludeStage::Type, "a.hexl"));
        assert_eq!(l.len(), 2);
    }

    #[test]
    fn keeps_registration_order() {
        let mut l = IncludeList::new();
        l.insert(IncludeStage::CFile, "b.c");
        l.insert(IncludeStage::Define, "x");
        l.insert(IncludeStage::CFile, "a.c");
        l.insert(IncludeStage::CFile, "b.c");
        assert_eq!(
            l.keys(IncludeStage::CFile).collect::<Vec<_>>(),
            ["b.c", "a.c"]
        );
    }

    #[test]
    fn only_and_merge_pass_one_stage() {
        let mut parent = IncludeList::new();
        parent.insert(IncludeStage::Type, "t1");
        parent.insert(IncludeStage::Include, "inc");

        // 子へは`Type`だけを渡す(親の`Include`で、子の`#include`が消えてはいけない)
        let mut child = parent.only(IncludeStage::Type);
        assert!(child.contains(IncludeStage::Type, "t1"));
        assert!(!child.contains(IncludeStage::Include, "inc"));

        child.insert(IncludeStage::Type, "t2");
        child.insert(IncludeStage::Include, "child-only");
        parent.merge(&child, IncludeStage::Type);

        assert!(parent.contains(IncludeStage::Type, "t2"));
        assert!(!parent.contains(IncludeStage::Include, "child-only"));
        assert_eq!(parent.len(), 3);
    }
}
