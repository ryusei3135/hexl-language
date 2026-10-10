/// 構造体や列挙型などのノードの定義
pub mod data_node;
/// テスト時のみ使う関数など
pub mod test_utils;
pub mod expr_node;
pub mod stmt_node;

use std::collections::HashMap;

pub(in crate::compiler) use data_node::*;
pub(in crate::compiler) use test_utils::*;
pub(in crate::compiler) use expr_node::*;
pub(in crate::compiler) use stmt_node::*;

use crate::compiler::{models::Body, node, parse::VarMutAttr};

pub const IS_MUST: usize = 0;
pub const IS_OF: usize = 1;

#[derive(Clone, Debug, PartialEq)]
pub struct ConstractTy {
    must: Option<String>,
    of: Option<String>,
    ty: Box<TyNode>,
}

impl ConstractTy {
    pub fn new<const K: usize>(
        must: Option<String>,
        of: Option<String>,
        ty: Box<TyNode>,
    ) -> TyNode {
        // 自クラス（TyNode）ではなく、内側の構造体を組み立てる
        let base = Self { must, of, ty };

        // constジェネリクスの文字列をifで判定する
        if K == IS_MUST {
            TyNode::ConstractMust(base)
        } else if K == IS_OF {
            TyNode::ConstractOf(base)
        } else {
            panic!("Unsupported key: {}", K);
        }
    }

    #[inline(always)]
    pub fn unwrap_ty(&self) -> TyNode {
        (*self.ty).clone()
    }

    /// `must`側から見た契約の名前
    /// (`must`が指定されていない場合は`of`の名前を使う)
    #[inline(always)]
    pub fn must_name(&self) -> Option<&String> {
        self.must.as_ref().or(self.of.as_ref())
    }

    /// `of`側から見た契約の名前
    /// (`of`が指定されていない場合は`must`の名前を使う)
    #[inline(always)]
    pub fn of_name(&self) -> Option<&String> {
        self.of.as_ref().or(self.must.as_ref())
    }

    /// エラーメッセージ用に、契約の名前を文字列化する
    pub fn name_or_anon<'a>(name: Option<&'a str>) -> &'a str {
        match name {
            Some(name) => name,
            None => "(名前なし)",
        }
    }
}
