use std::collections::HashMap;

use crate::models::Body;


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
        ty: Box<TyNode>
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
    pub fn name_or_anon<'a>(
        name: Option<&'a String>
    ) -> &'a str {
        match name {
            Some(name) => name.as_str(),
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
            Self::ConstractMust(ty) | Self::ConstractOf(ty) => {
                ty.unwrap_ty().get_ty_str_name()
            }
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
            Self::ConstractMust(ty) 
            | Self::ConstractOf(ty) => ty.unwrap_ty(),
            t => t.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ArgsNode {
    pub name: String,
    pub ty: TyNode,
    pub is_mut: bool,
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
    pub fn new(
        name: String, 
        args: Vec<ArgsNode>, 
        ret_ty: TyNode, 
        public: bool
    ) -> Group1Node {
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
    pub fn set_temp_ty(
        &mut self, 
        temp_ty: Vec<TyNode>
    ) {
        self.temp_ty = temp_ty;
    }

    pub fn self_module_name(
        &mut self, 
        name: &String
    ) {
        self.module = Some(name.to_string());
    }

    pub fn add(&mut self, node: Group2Info) {
        self.body.push(node);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StructField {
    pub name: String,
    pub ty: TyNode,
}

impl StructField {
    #[cfg(test)]
    pub fn make_field(name: &str, ty: &str) -> Self {
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
        methods: Vec<Group1Node>
    ) -> Group1Node {
        Group1Node::StructDefine(Self {
            name,
            fields,
            methods,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnumDefine {
    pub name: String,
    pub variants: Vec<String>,
}

impl EnumDefine {
    #[inline(always)]
    pub fn new(
        name: String, 
        variants: Vec<String>
    ) -> Group1Node {
        Group1Node::EnumDefine(
            Self { 
                name, 
                variants 
            }
        )
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
pub struct DefineVar {
    pub name: String,
    pub value: Box<Expr>,
    pub ty: TyNode,
    pub is_mut: bool,
}

impl DefineVar {
    pub fn new(
        name: &String, 
        value: Expr, 
        ty: &TyNode,
        is_mut: &bool
    ) -> Self {
        Self {
            name: name.to_string(),
            value: Box::new(value),
            ty: ty.clone(),
            is_mut: *is_mut,
        }
    }

    pub fn wrap(self) -> Expr {
        Expr::DefVar(self)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MatchArm {
    pub pattern: Box<Expr>,
    pub body: Body,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AssignVar {
    pub name: String,
    // Expr::Varなど
    pub dst: Box<Expr>,
    pub value: Box<Expr>,
}

impl AssignVar {
    #[inline(always)]
    pub fn new(
        name: &String, 
        dst: Expr, 
        value: Expr
    ) -> Expr {
        Expr::Assign(Self {
            name: name.to_string(),
            dst: Box::new(dst),
            value: Box::new(value),
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    RangeNode((String, String)),
    /// ポインタ関係
    GetAddress(Box<Expr>),
    ConnectAddr(Box<Expr>),

    Number(String),
    Str(String),
    Var(String),
    CallFunc(CallInfo),
    Assign(AssignVar),
    Add((Box<Expr>, Box<Expr>)),
    Sub((Box<Expr>, Box<Expr>)),
    Mul((Box<Expr>, Box<Expr>)),
    Div((Box<Expr>, Box<Expr>)),
    Surplus((Box<Expr>, Box<Expr>)),
    LessThen((Box<Expr>, Box<Expr>)),
    GreaterThen((Box<Expr>, Box<Expr>)),
    /// `==` 値の等価比較
    Equal((Box<Expr>, Box<Expr>)),
    NotEq((Box<Expr>, Box<Expr>)),
    Match {
        pattern: Option<Box<Expr>>,
        arms: Vec<MatchArm>,
        arm_else: Option<Body>,
    },
    Loop {
        pattern: Option<Box<Expr>>,
        body: Body,
    },
    InitStruct {
        is_self: bool,
        name: String,
        fields: HashMap<String, Box<Expr>>,
    },
    /// 列挙型のメンバへのアクセス: `Name::Mem`
    EnumVariant {
        name: String,
        variant: String,
    },
    /// 配列リテラル: `{100, 100, 100, 100}`
    Array(Vec<Expr>),
    /// 配列の要素へのアクセス(参照/代入どちらの対象にもなる)
    /// `[name index]`
    /// - name: 配列変数の名前
    /// - dst: 配列本体を指す式
    /// - index: 添字を表す式(`node::Expr::Number`など)
    RefArray {
        name: String,
        dst: Box<Expr>,
        index: Box<Expr>,
    },

    DefVar(DefineVar),
    Scope {
        scope: Vec<String>,
        target: Box<Expr>,
    },
    Member {
        scope: Vec<String>,
        target: Box<Expr>,
    },
}

impl Expr {
    pub fn wrap(
        left: Expr, 
        right: Expr
    ) -> (Box<Expr>, Box<Expr>) {
        (Box::new(left), Box::new(right))
    }

    pub fn get_assign_node_name(&self) -> String {
        match &self {
            Self::Assign(assign_node) => {
                assign_node.clone().name
            }
            _ => panic!(),
        }
    }

    pub fn get_assign_node(&mut self) -> &mut AssignVar {
        match self {
            Self::Assign(ref mut name) => {
                return name;
            }
            _ => panic!(),
        }
    }

    pub fn wrap_group2(self) -> Group2Node {
        Group2Node::Expr(self)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModPath {
    pub path: Vec<String>,
}

impl ModPath {
    #[inline(always)]
    pub const fn new() -> Self {
        Self { path: Vec::new() }
    }

    #[inline(always)]
    pub fn add_path(
        &mut self, 
        path_name: &String
    ) {
        self.path.push(path_name.clone());
    }

    pub fn gen_path(&self) -> String {
        // ディレクトリの最初のパスのインデック
        const PATH_START: usize = 0;

        let mut path = String::new();
        for (index, dir) in self.path.iter().enumerate() {
            if index != PATH_START {
                path.push('/');
            }
            path.push_str(dir);
        }
        // 最後に拡張子を追加
        path.push_str(".hexl");
        path
    }

    /// パスの最後のセグメントを除いた、
    /// 親のディレクトリのファイルパスを生成する
    /// (例: `mod::file::func` -> `mod/file.hexl`)
    ///
    /// `#include`で指定されたパスがファイルとして
    /// 存在しない場合、最後のセグメントは
    /// 関数名とみなし、その手前までを
    /// ファイルパスとして探すのに使う
    pub fn gen_parent_path(&self) -> String {
        const PATH_START: usize = 0;

        let mut path = String::new();
        let parent_len = self.path
            .len()
            .saturating_sub(1);
        for (index, dir) in self.path[..parent_len].iter().enumerate() {
            if index != PATH_START {
                path.push('/');
            }
            path.push_str(dir);
        }
        path.push_str(".hexl");
        path
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CallInfo {
    pub name: String,
    /// `func<int>(..)`の`<>`の中身。呼び出す関数を
    /// `FuncDefine`の`temp_ty`と突き合わせて特定するために使う。
    /// ジェネリクスでない関数の呼び出しでは空
    pub temp_ty: Vec<TyNode>,
    pub args: Vec<Expr>,
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
    pub fn get_node<'a>(
        &'a self
    ) -> &'a Group2Node {
        &self.node
    }
}


impl Group2Node {
    /// ノードに、何行目かの情報を入れる
    #[inline(always)]
    pub fn gen_group_info(
        self, 
        line: &usize
    ) -> Group2Info {
        Group2Info { 
            line: *line, 
            node: self
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
    EnumDefine(EnumDefine),
    Include(ModPath),
    Line(String),
}




#[cfg(test)]
pub fn gen_var_node(
    name: &str,
    value: &str,
    ty: &str,
    line: usize,
) -> Group2Info {
    Group2Node::Expr(Expr::DefVar(DefineVar::new(
        &name.to_string(),
        Expr::Number(value.to_string()),
        &TyNode::Ty(ty.to_string()),
        &false
    )))
    .gen_group_info(&line)
}

#[cfg(test)]
pub fn wrap_expr_cmp(left: &str, right: &str) -> Expr {
    Expr::LessThen((
        Box::new(Expr::Number(left.to_string())),
        Box::new(Expr::Number(right.to_string())),
    ))
}

#[cfg(test)]
pub fn wrap_eq_expr_cmp(left: &str, right: &str) -> Expr {
    Expr::Equal((
        Box::new(Expr::Number(left.to_string())),
        Box::new(Expr::Number(right.to_string())),
    ))
}
