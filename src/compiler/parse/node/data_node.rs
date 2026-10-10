//! 構造体などのデータ型のノードの定義

// use crate::compiler::node::VariantMode::EnumMode;

use super::*;


pub(in crate::compiler)
trait Field {
    /// 所有権で渡す
    fn new(name: String, ty: TyNode) -> Self;

    fn make_field(name: &str, ty: &str) -> Self;
}


// === 構造体 =====

#[derive(Clone, Debug, PartialEq)]
pub struct StructField {
    pub name: String,
    pub ty: TyNode,
}

impl Field for StructField {
    fn new(name: String, ty: TyNode) -> Self {
        Self {
            name,
            ty,
        }
    }

    fn make_field(name: &str, ty: &str) -> Self {
        Self {
            name: name.to_string(),
            ty: TyNode::Ty(ty.to_string()),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StructDefine {
    pub name: String,
    pub fields: Vec<StructField>,
    pub methods: Vec<Group1Node>,
}

impl StructDefine {
    #[inline(always)]
    pub const fn new(
        name: String,
        fields: Vec<StructField>,
        methods: Vec<Group1Node>,
    ) -> Group1Node {
        Group1Node::StructDefine(Self {
            name,
            fields,
            methods,
        })
    }
}

// ===== バリアント型(旧共用体 + 旧列挙型) =====

#[derive(Clone, Debug, PartialEq)]
pub enum VariantMode {
    Normal,
    Unsafe,
}

/// バリアント型の中身(フィールドかメゾット)
#[derive(Clone, Debug, PartialEq)]
pub enum VariantFieldKind {
    Field(VariantField),
    Method(Group1Node),
}

#[derive(Clone, Debug, PartialEq)]
pub struct VariantField {
    pub name: String,
    pub ty: Option<TyNode>,
    pub mode: VariantMode,
}

impl Field for VariantField {
    fn new(name: String, ty: TyNode) -> Self {
        Self {
            name,
            ty: Some(ty),
            mode: VariantMode::Normal,
        }
    }

    fn make_field(name: &str, ty: &str) -> Self {
        Self {
            name: name.to_string(),
            ty: Some(TyNode::Ty(ty.to_string())),
            mode: VariantMode::Normal,
        }
    }
}

impl VariantField {
    /// `A::Name`のように`Normal`かつ型を指定しないときに使う
    /// ```
    /// variant A {
    ///     Name
    ///     Name1(int)
    /// }
    /// ```
    pub fn typeless_new(name: String) -> Self {
        Self {
            name,
            ty: None,
            mode: VariantMode::Normal,
        }
    }

    /// `Unsafe`モードのフィールドを作る
    pub fn unsafe_new(name: String, ty: TyNode) -> Self {
        Self {
            name,
            ty: Some(ty),
            mode: VariantMode::Unsafe,
        }
    }

    pub fn wrap_variant_field_kind(self) -> VariantFieldKind {
        VariantFieldKind::Field(self)
    }
}


#[derive(Clone, Debug, PartialEq)]
pub struct VariantDefine {
    pub name: String,
    pub fields: Vec<VariantField>,
    pub methods: Vec<Group1Node>,
}

impl VariantDefine {
    #[inline(always)]
    pub const fn new(
        name: String,
        fields: Vec<VariantField>,
        methods: Vec<Group1Node>,
    ) -> Group1Node {
        Group1Node::VariantDefine(Self {
            name,
            fields,
            methods,
        })
    }

    /// `VariantFieldKind`の一覧から、フィールドとメゾットに振り分けて作る
    pub fn from_kinds(name: String, kinds: Vec<VariantFieldKind>) -> Group1Node {
        let mut fields = Vec::new();
        let mut methods = Vec::new();
        for kind in kinds {
            match kind {
                VariantFieldKind::Field(f) => fields.push(f),
                VariantFieldKind::Method(m) => methods.push(m),
            }
        }
        Self::new(name, fields, methods)
    }

    /// タグ付き共用体(`Normal`)かどうか。
    /// `Unsafe`のフィールドが1つでもあればタグを持たない共用体として扱う
    pub fn is_tagged(&self) -> bool {
        !self.fields.iter().any(|f| f.mode == VariantMode::Unsafe)
    }

    /// 全メンバーが型なしの`Normal`か(旧`enum`相当、C言語風の列挙型)
    pub fn is_plain_enum(&self) -> bool {
        self.fields
            .iter()
            .all(|f| f.ty.is_none() && f.mode == VariantMode::Normal)
    }

    /// 名前からフィールドを探す
    pub fn find_field(&self, name: &str) -> Option<&VariantField> {
        self.fields.iter().find(|f| f.name == name)
    }

    /// フィールドのインデックス(定義順)をタグとして返す
    pub fn tag_of(&self, name: &str) -> Option<usize> {
        self.fields.iter().position(|f| f.name == name)
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
