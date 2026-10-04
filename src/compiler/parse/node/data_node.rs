//! 構造体などのデータ型のノードの定義

use super::*;


pub(in crate::compiler::parse) 
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
pub struct UnionField {
    pub name: String,
    pub ty: TyNode,
}

impl Field for UnionField {
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