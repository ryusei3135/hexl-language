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

#[derive(Clone, Debug, PartialEq)]
pub enum TyNode {
    Ty(String),
    /// メゾット内で使われる`Self`(自身の構造体を表す予約語)
    /// - 内側の`String`は、`Self`が実際に指している構造体の名前
    SelfTy(String),
    /// ポインタ
    Pointer {
        /// 不変ポインタの場合true
        is_const: bool,
        ty_name: Box<TyNode>,
        range: Option<(usize, usize)>,
    },
    /// 参照の変数
    RefTy(Box<TyNode>),
    /// スタック領域に確保する変数の型
    /// `name: [ty] = 100` / `name: [ty 4] = {100, 100, 100, 100}`
    /// - name: 要素の型名
    /// - len: 要素数(指定が無ければ1)
    Stack {
        name: String,
        len: usize,
    },
    /// 静的領域に確保する変数の型
    /// `name: ""[ty] = 100`
    /// スタックと同じ意味を持つが、確保される場所が静的領域になる
    Static {
        name: String,
        len: usize,
    },

    ConstractMust(ConstractTy),
    ConstractOf(ConstractTy),
}

impl TyNode {
    pub fn make_ref_ty(ty_name: &String) -> Self {
        Self::RefTy(Box::new(Self::Ty(ty_name.to_string())))
    }

    pub fn get_ty_str_name(&self) -> String {
        match self {
            Self::Ty(name) => name.to_string(),
            Self::SelfTy(name) => name.to_string(),
            Self::RefTy(t) => t.get_ty_str_name(),
            Self::Stack { name, .. } => name.to_string(),
            Self::Static { name, .. } => name.to_string(),
            Self::Pointer { ty_name, .. } => ty_name.get_ty_str_name(),
            Self::ConstractMust(ty) | Self::ConstractOf(ty) => ty.unwrap_ty().get_ty_str_name(),
        }
    }

    /// `must`の契約が付いた型なら、その契約の情報を返す
    #[inline(always)]
    pub fn as_constract_must(&self) -> Option<&ConstractTy> {
        match self {
            Self::ConstractMust(ty) => Some(ty),
            _ => None,
        }
    }

    /// `of`の契約が付いた型なら、その契約の情報を返す
    #[inline(always)]
    pub fn as_constract_of(&self) -> Option<&ConstractTy> {
        match self {
            Self::ConstractOf(ty) => Some(ty),
            _ => None,
        }
    }

    #[inline(always)]
    pub fn is_constract_must(&self) -> bool {
        self.as_constract_must().is_some()
    }

    #[inline(always)]
    pub fn is_constract_of(&self) -> bool {
        self.as_constract_of().is_some()
    }

    /// 契約(`must`/`of`)が付いている場合は、その内側の型を返す
    /// 付いていない場合は自分自身をそのまま返す
    pub fn unwrap_constract(&self) -> TyNode {
        match self {
            Self::ConstractMust(ty) | Self::ConstractOf(ty) => ty.unwrap_ty(),
            t => t.clone(),
        }
    }
}
