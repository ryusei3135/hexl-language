use crate::compiler::{
    ast::FuncDefInfo::{AllDef, DefOnly},
    err,
};

use super::*;
use std::rc::Rc;

/// 各ノードが持つ、ソース上の位置
type NodeSpan = Rc<crate::compiler::err::Span>;

/// 関数のノードが中身まで定義されているか
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FuncDefInfo {
    /// 関数の定義だけの場合、中身はない
    DefOnly,
    /// 中身まですべて定義されている
    AllDef,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArgsNode {
    pub name: String,
    pub ty: TyNode,
    pub var_attr: VarMutAttr,
    pub span: NodeSpan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FuncDefine {
    pub public: bool,
    pub name: String,
    /// ジェネリクス関数(`func<T>(..)`)から作られた関数の場合の、
    /// `<>`の中身(呼び出しで指定された実際の型)。
    /// `func<int>()`なら`[Ty("int")]`。ジェネリクスでない関数は空
    pub temp_ty: Vec<TyNode>,
    pub params: Vec<ArgsNode>,
    pub ret_ty: TyNode,
    pub body: Vec<Group2Info>,
    pub module: Option<String>,
    def_info: FuncDefInfo,
    pub span: NodeSpan,
}

impl FuncDefine {
    pub fn new(
        name: String,
        args: Vec<ArgsNode>,
        ret_ty: TyNode,
        public: bool,
        span: NodeSpan,
    ) -> Group1Node {
        Group1Node::FuncDefine(Self {
            public,
            name: name,
            temp_ty: Vec::new(),
            params: args,
            ret_ty: ret_ty,
            body: Vec::new(),
            module: None,
            def_info: FuncDefInfo::DefOnly,
            span,
        })
    }

    /// ジェネリクス関数から作った関数に、`<>`の中身を登録する
    #[inline(always)]
    pub fn set_temp_ty(&mut self, temp_ty: &[TyNode]) {
        self.temp_ty = temp_ty.to_vec();
    }

    pub fn self_module_name(&mut self, name: String) {
        self.module = Some(name);
    }

    pub fn add(&mut self, node: Group2Info) {
        // 関数の処理が追加された
        if self.def_info == DefOnly {
            self.def_info = AllDef;
        }
        self.body.push(node);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StmtNode {
    Return(Expr),
    Continue,
    Break(Rc<err::Span>),
}

impl StmtNode {
    pub fn wrap(self) -> Group2Node {
        Group2Node::Stmt(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchArm {
    pub pattern: Box<Expr>,
    pub body: Body,
    pub span: NodeSpan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InlineAsm {
    /// `${...}` の部分が `{0}`, `{1}`, ... のような
    /// プレースホルダーに置き換えられたアセンブリの文字列
    pub asm: String,
    /// プレースホルダーに対応する式(出現順、`{n}` <-> `operands[n]`)
    pub operands: Vec<Expr>,
    pub span: NodeSpan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Group2Node {
    Stmt(StmtNode),
    Expr(Expr),
    CompleSyntax((String, Vec<InlineAsm>)),
    Include(ModPath),
    Line(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group2Info {
    pub line: usize,
    pub node: Group2Node,
    pub span: NodeSpan,
}

impl Group2Info {
    /// ノードを抽出する
    #[inline(always)]
    pub fn get_node<'a>(&'a self) -> &'a Group2Node {
        &self.node
    }
}

impl Group2Node {
    /// ノードに、何行目かの情報を入れる
    #[inline(always)]
    pub fn gen_group_info(self, line: usize, span: NodeSpan) -> Group2Info {
        Group2Info {
            line: line,
            node: self,
            span,
        }
    }

    /// `Group1Node`へ変換する。`Include`と`Line`以外は対応していないので`Err`を返す
    pub fn change_group1(self) -> Result<Group1Node, crate::compiler::err::ErrKind> {
        match self {
            Self::Include(v) => Ok(Group1Node::Include(v)),
            Self::Line(v) => Ok(Group1Node::Line(v)),
            _ => Err(crate::compiler::err::ErrKind::UnexpectedToken),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Group1Node {
    FuncDefine(FuncDefine),
    StructDefine(StructDefine),
    VariantDefine(VariantDefine),
    TypeAlias(TypeAlias),
    Include(ModPath),
    Line(String),
}

impl Group1Node {
    pub fn wrap_variant_field_kind(self) -> VariantFieldKind {
        VariantFieldKind::Method(self)
    }

    /// ノードに、モジュールのパスを(外側として)足す
    ///
    /// 空のパスなら何もしない。`Include`と`Line`はモジュールを持たないので何もしない
    pub fn add_mod_path(&mut self, path: Vec<String>) {
        if path.is_empty() {
            return;
        }
        match self {
            Self::FuncDefine(n) => Self::prepend_mod_path(&mut n.module, &path),
            Self::StructDefine(n) => Self::prepend_mod_path(&mut n.module, &path),
            Self::VariantDefine(n) => Self::prepend_mod_path(&mut n.module, &path),
            Self::TypeAlias(n) => Self::prepend_mod_path(&mut n.module, &path),
            Self::Include(_) | Self::Line(_) => {}
        }
    }

    /// ノードに`pub`が付いているかを設定する
    ///
    /// `Include`と`Line`は公開/非公開を持たないので何もしない。
    /// `new`で作ったノードは非公開(`false`)なので、`pub`を付けて定義された
    /// 構造体/バリアント/型エイリアスにだけ`set_public(true)`を呼ぶ
    pub fn set_public(&mut self, public: bool) {
        match self {
            Self::FuncDefine(n) => n.public = public,
            Self::StructDefine(n) => n.public = public,
            Self::VariantDefine(n) => n.public = public,
            Self::TypeAlias(n) => n.public = public,
            Self::Include(_) | Self::Line(_) => {}
        }
    }

    /// ノードに`pub`が付いているか(`Include`と`Line`は常に`false`)
    pub fn is_public(&self) -> bool {
        match self {
            Self::FuncDefine(n) => n.public,
            Self::StructDefine(n) => n.public,
            Self::VariantDefine(n) => n.public,
            Self::TypeAlias(n) => n.public,
            Self::Include(_) | Self::Line(_) => false,
        }
    }

    /// ノードにモジュールのパスを一つ追加する
    /// func -> mod::func
    pub fn add_mod_name(&mut self, name: String) {
        self.add_mod_path(vec![name]);
    }

    /// 現在のモジュール(`Option<String>`)の外側に、パスを足す
    /// 現在が`b`で`["a", "x"]`なら`a::x::b`、現在がなければ`a::x`
    fn prepend_mod_path(module: &mut Option<String>, path: &[String]) {
        let mut full = path.join("::");
        if let Some(cur) = module.take() {
            full.push_str("::");
            full.push_str(&cur);
        }
        *module = Some(full);
    }
}
