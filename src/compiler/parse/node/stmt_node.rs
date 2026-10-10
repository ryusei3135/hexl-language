use super::*;


#[derive(Clone, Debug, PartialEq)]
pub struct ArgsNode {
    pub name: String,
    pub ty: TyNode,
    pub var_attr: VarMutAttr,
}

#[derive(Clone, Debug, PartialEq)]
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
}

impl FuncDefine {
    pub fn new(name: String, args: Vec<ArgsNode>, ret_ty: TyNode, public: bool) -> Group1Node {
        Group1Node::FuncDefine(Self {
            public,
            name: name,
            temp_ty: Vec::new(),
            params: args,
            ret_ty: ret_ty,
            body: Vec::new(),
            module: None,
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
        self.body.push(node);
    }
}


#[derive(Clone, Debug, PartialEq)]
pub enum StmtNode {
    Return(Expr),
    Continue,
    Break,
}

impl StmtNode {
    pub fn wrap(self) -> Group2Node {
        Group2Node::Stmt(self)
    }
}


#[derive(Clone, Debug, PartialEq)]
pub struct MatchArm {
    pub pattern: Box<Expr>,
    pub body: Body,
}


#[derive(Clone, Debug, PartialEq)]
pub struct InlineAsm {
    /// `${...}` の部分が `{0}`, `{1}`, ... のような
    /// プレースホルダーに置き換えられたアセンブリの文字列
    pub asm: String,
    /// プレースホルダーに対応する式(出現順、`{n}` <-> `operands[n]`)
    pub operands: Vec<Expr>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Group2Node {
    Stmt(StmtNode),
    Expr(Expr),
    CompleSyntax((String, Vec<InlineAsm>)),
    Include(ModPath),
    Line(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Group2Info {
    pub line: usize,
    pub node: Group2Node,
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
    pub fn gen_group_info(self, line: usize) -> Group2Info {
        Group2Info {
            line: line,
            node: self,
        }
    }

    pub fn change_group1(self) -> Group1Node {
        match self {
            Self::Include(v) => Group1Node::Include(v),
            Self::Line(v) => Group1Node::Line(v),
            t => panic!("{:?} <- これは対応していません", t),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Group1Node {
    FuncDefine(FuncDefine),
    StructDefine(StructDefine),
    VariantDefine(VariantDefine),
    Include(ModPath),
    Line(String),
}

impl Group1Node {
    pub fn wrap_variant_field_kind(self) -> VariantFieldKind {
        VariantFieldKind::Method(self)
    }
}