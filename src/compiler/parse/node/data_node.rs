//! 構造体などのデータ型のノードの定義

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

// ===== 共用体 =====

#[derive(Clone, Debug, PartialEq)]
pub enum UnionMode {
    Normal,
    Unsafe,
}

/// 共用体の中身(フィールドかメゾット)
#[derive(Clone, Debug, PartialEq)]
pub enum UnionFieldKind {
    Field(UnionField),
    Method(Group1Node),
}

#[derive(Clone, Debug, PartialEq)]
pub struct UnionField {
    pub name: String,
    pub ty: Option<TyNode>,
    pub mode: UnionMode,
}

impl Field for UnionField {
    fn new(name: String, ty: TyNode) -> Self {
        Self {
            name,
            ty: Some(ty),
            mode: UnionMode::Normal,
        }
    }

    fn make_field(name: &str, ty: &str) -> Self {
        Self {
            name: name.to_string(),
            ty: Some(TyNode::Ty(ty.to_string())),
            mode: UnionMode::Normal,
        }
    }
}

impl UnionField {
    /// `A::Name`のように`Normal`かつ型を指定しないときに使う
    /// ```
    /// union A {
    ///     Name
    ///     Name1(int)
    /// }
    /// ```
    pub fn typeless_new(name: String) -> Self {
        Self {
            name,
            ty: None,
            mode: UnionMode::Normal,
        }
    }

    /// `Unsafe`モードのフィールドを作る
    pub fn unsafe_new(name: String, ty: TyNode) -> Self {
        Self {
            name,
            ty: Some(ty),
            mode: UnionMode::Unsafe,
        }
    }

    pub fn wrap_union_field_kind(self) -> UnionFieldKind {
        UnionFieldKind::Field(self)
    }
}


#[derive(Clone, Debug, PartialEq)]
pub struct UnionDefine {
    pub name: String,
    pub fields: Vec<UnionField>,
    pub methods: Vec<Group1Node>,
}

impl UnionDefine {
    #[inline(always)]
    pub const fn new(
        name: String,
        fields: Vec<UnionField>,
        methods: Vec<Group1Node>,
    ) -> Group1Node {
        Group1Node::UnionDefine(Self {
            name,
            fields,
            methods,
        })
    }

    /// `UnionFieldKind`の一覧から、フィールドとメゾットに振り分けて作る
    pub fn from_kinds(name: String, kinds: Vec<UnionFieldKind>) -> Group1Node {
        let mut fields = Vec::new();
        let mut methods = Vec::new();
        for kind in kinds {
            match kind {
                UnionFieldKind::Field(f) => fields.push(f),
                UnionFieldKind::Method(m) => methods.push(m),
            }
        }
        Self::new(name, fields, methods)
    }

    /// タグ付き共用体(`Normal`)かどうか。
    /// `Unsafe`のフィールドが1つでもあればタグを持たない共用体として扱う
    pub fn is_tagged(&self) -> bool {
        !self.fields.iter().any(|f| f.mode == UnionMode::Unsafe)
    }

    /// 名前からフィールドを探す
    pub fn find_field(&self, name: &str) -> Option<&UnionField> {
        self.fields.iter().find(|f| f.name == name)
    }

    /// フィールドのインデックス(定義順)をタグとして返す
    pub fn tag_of(&self, name: &str) -> Option<usize> {
        self.fields.iter().position(|f| f.name == name)
    }
}

// ===== 列挙型 =====

#[derive(Clone, Debug, PartialEq)]
pub struct EnumDefine {
    pub name: String,
    pub variants: Vec<String>,
}

impl EnumDefine {
    #[inline(always)]
    pub fn new(name: String, variants: Vec<String>) -> Group1Node {
        Group1Node::EnumDefine(Self { name, variants })
    }
}