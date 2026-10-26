//! 構造体などのデータ型のノードの定義

// use crate::compiler::ast::VariantMode::EnumMode;

use crate::compiler::err;

use super::*;

pub(in crate::compiler) trait Field {
    /// 所有権で渡す
    fn new(name: String, ty: TyNode, span: Rc<err::Span>) -> Self;

    fn make_field(name: &str, ty: &str, span: Rc<err::Span>) -> Self;
}

// === 型エイリアス =====

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeAlias {
    pub name: String,
    pub ty: TyNode,
    /// 所属するモジュールのパス(`a::b`)
    pub module: Option<String>,
    /// `type`の位置
    pub span: Rc<err::Span>,
}

impl TypeAlias {
    #[inline(always)]
    pub const fn new(name: String, ty: TyNode, span: Rc<err::Span>) -> Group1Node {
        Group1Node::TypeAlias(Self {
            name,
            ty,
            module: None,
            span,
        })
    }
}

// === 構造体 =====

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StructField {
    pub name: String,
    pub ty: TyNode,
    pub span: Rc<err::Span>,
}

impl Field for StructField {
    fn new(name: String, ty: TyNode, span: Rc<err::Span>) -> Self {
        Self { name, ty, span }
    }

    fn make_field(name: &str, ty: &str, span: Rc<err::Span>) -> Self {
        Self {
            name: name.to_string(),
            ty: TyNode::Ty(ty.to_string()),
            span,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StructDefine {
    pub name: String,
    pub fields: Vec<StructField>,
    pub methods: Vec<Group1Node>,
    /// 所属するモジュールのパス(`a::b`)
    pub module: Option<String>,
    /// `struct Name`の位置
    pub span: Rc<err::Span>,
}

impl StructDefine {
    #[inline(always)]
    pub const fn new(
        name: String,
        fields: Vec<StructField>,
        methods: Vec<Group1Node>,
        span: Rc<err::Span>,
    ) -> Group1Node {
        Group1Node::StructDefine(Self {
            name,
            fields,
            methods,
            module: None,
            span,
        })
    }
}

// ===== バリアント型(旧共用体 + 旧列挙型) =====

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VariantMode {
    Normal,
    Unsafe,
}

/// バリアント型の中身(フィールドかメゾット)
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VariantFieldKind {
    Field(VariantField),
    Method(Group1Node),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VariantField {
    pub name: String,
    pub ty: Option<TyNode>,
    pub mode: VariantMode,
    pub span: Rc<err::Span>,
}

impl Field for VariantField {
    fn new(name: String, ty: TyNode, span: Rc<err::Span>) -> Self {
        Self {
            name,
            ty: Some(ty),
            mode: VariantMode::Normal,
            span,
        }
    }

    fn make_field(name: &str, ty: &str, span: Rc<err::Span>) -> Self {
        Self {
            name: name.to_string(),
            ty: Some(TyNode::Ty(ty.to_string())),
            mode: VariantMode::Normal,
            span,
        }
    }
}

impl VariantField {
    /// `A::Name`のように`Normal`かつ型を指定しないときに使う
    /// ```txt
    /// variant A {
    ///     Name
    ///     Name1(int)
    /// }
    /// ```
    pub fn typeless_new(name: String, span: Rc<err::Span>) -> Self {
        Self {
            name,
            ty: None,
            mode: VariantMode::Normal,
            span,
        }
    }

    // /// `Unsafe`モードのフィールドを作る
    // pub fn unsafe_new(name: String, ty: TyNode, span: Rc<err::Span>) -> Self {
    //     Self {
    //         name,
    //         ty: Some(ty),
    //         mode: VariantMode::Unsafe,
    //         span,
    //     }
    // }

    pub fn wrap_variant_field_kind(self) -> VariantFieldKind {
        VariantFieldKind::Field(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VariantDefine {
    pub name: String,
    pub fields: Vec<VariantField>,
    pub methods: Vec<Group1Node>,
    /// 所属するモジュールのパス(`a::b`)
    pub module: Option<String>,
    /// `variant Name`の位置
    pub span: Rc<err::Span>,
}

impl VariantDefine {
    #[inline(always)]
    pub const fn new(
        name: String,
        fields: Vec<VariantField>,
        methods: Vec<Group1Node>,
        span: Rc<err::Span>,
    ) -> Group1Node {
        Group1Node::VariantDefine(Self {
            name,
            fields,
            methods,
            module: None,
            span,
        })
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TyNode {
    Auto,
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
        name: Box<TyNode>,
        len: usize,
    },
    /// 静的領域に確保する変数の型
    /// `name: ""[ty] = 100`
    /// スタックと同じ意味を持つが、確保される場所が静的領域になる
    Static {
        name: Box<TyNode>,
        len: usize,
    },

    ConstractMust(ConstractTy),
    ConstractOf(ConstractTy),
}

impl TyNode {
    /// 型名(`Ty`)を`lookup`で置き換えた新しい型を返す。
    /// ポインタ・参照・配列・契約の中の型も再帰的に置き換える。
    /// `lookup`は型名から展開後の型を返す(型エイリアスなら`Some`)
    pub fn expand_alias(&self, lookup: &dyn Fn(&str) -> Option<TyNode>) -> TyNode {
        match self {
            Self::Ty(name) => lookup(name).unwrap_or_else(|| self.clone()),
            Self::SelfTy(_) => self.clone(),
            Self::Pointer {
                is_const,
                ty_name,
                range,
            } => Self::Pointer {
                is_const: *is_const,
                ty_name: Box::new(ty_name.expand_alias(lookup)),
                range: *range,
            },
            Self::RefTy(t) => Self::RefTy(Box::new(t.expand_alias(lookup))),
            Self::Stack { name, len } => Self::Stack {
                name: Box::new(name.expand_alias(lookup)),
                len: *len,
            },
            Self::Static { name, len } => Self::Static {
                name: Box::new(name.expand_alias(lookup)),
                len: *len,
            },
            Self::ConstractMust(c) => {
                Self::ConstractMust(c.with_ty(c.unwrap_ty().expand_alias(lookup)))
            }
            Self::ConstractOf(c) => {
                Self::ConstractOf(c.with_ty(c.unwrap_ty().expand_alias(lookup)))
            }
            Self::Auto => panic!(),
        }
    }

    pub fn make_ref_ty(ty_name: &String) -> Self {
        Self::RefTy(Box::new(Self::Ty(ty_name.to_string())))
    }

    pub fn get_ty_str_name(&self) -> String {
        match self {
            Self::Ty(name) => name.to_string(),
            Self::SelfTy(name) => name.to_string(),
            Self::RefTy(t) => t.get_ty_str_name(),
            Self::Stack { name, .. } => name.get_ty_str_name(),
            Self::Static { name, .. } => name.get_ty_str_name(),
            Self::Pointer { ty_name, .. } => ty_name.get_ty_str_name(),
            Self::ConstractMust(ty) | Self::ConstractOf(ty) => ty.unwrap_ty().get_ty_str_name(),
            Self::Auto => panic!(),
        }
    }

    /// `must`の契約が付いた型なら、その契約の情報を返す
    #[inline(always)]
    pub fn as_contract_must(&self) -> Option<&ConstractTy> {
        match self {
            Self::ConstractMust(ty) => Some(ty),
            _ => None,
        }
    }

    /// `of`の契約が付いた型なら、その契約の情報を返す
    #[inline(always)]
    pub fn as_contract_of(&self) -> Option<&ConstractTy> {
        match self {
            Self::ConstractOf(ty) => Some(ty),
            _ => None,
        }
    }

    #[inline(always)]
    pub fn is_contract_must(&self) -> bool {
        self.as_contract_must().is_some()
    }

    #[inline(always)]
    pub fn is_contract_of(&self) -> bool {
        self.as_contract_of().is_some()
    }
}
