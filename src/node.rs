use std::collections::HashMap;


#[derive(Clone, Debug, PartialEq)]
pub enum TyNode {
    Ty(String),
    /// ポインタ型
    Pointer{
        /// 不変ポインタの場合true
        is_const: bool,
        ty_name: Box<TyNode>
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
}

impl TyNode {
    pub fn make_ref_ty(ty_name: &String) -> Self {
        Self::RefTy(Box::new(Self::Ty(ty_name.to_string())))
    }

    pub fn get_ty_str_name(&self) -> String {
        match self {
            Self::Ty(name) => name.to_string(),
            Self::RefTy(t) => t.get_ty_str_name(),
            Self::Stack { name, ..} => name.to_string(),
            Self::Static { name, .. } => name.to_string(),
            Self::Pointer { ty_name, .. } => ty_name.get_ty_str_name()
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ArgsNode {
    pub name: String,
    pub ty: TyNode,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FuncDefine {
    pub public: bool,
    pub name: String,
    pub params: Vec<ArgsNode>,
    pub ret_ty: TyNode,
    pub body: Vec<Group2Node>,
    pub module: Option<String>,
}


impl FuncDefine {
    pub fn new(
        name: String,
        args: Vec<ArgsNode>,
        ret_ty: TyNode,
        public: bool,
    ) -> Group1Node {
        Group1Node::FuncDefine(
            Self {
                public,
                name: name,
                params: args,
                ret_ty: ret_ty,
                body: Vec::new(),
                module: None,
            }
        )
    }

    pub fn self_module_name(&mut self, name: &String) {
        self.module = Some(name.to_string());
    }

    pub fn add(&mut self, node: Group2Node) {
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
            ty: TyNode::Ty(ty.to_string())
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StructDefine {
    pub name: String,
    pub fields: Vec<StructField>,
    pub methods: Vec<Group1Node>
}

impl StructDefine {
    pub fn new(
        name: String,
        fields: Vec<StructField>,
        methods: Vec<Group1Node>
    ) -> Group1Node {
        Group1Node::StructDefine(
            Self { name, fields, methods }
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnumDefine {
    pub name: String,
    pub variants: Vec<String>,
}

impl EnumDefine {
    pub fn new(name: String, variants: Vec<String>) -> Group1Node {
        Group1Node::EnumDefine(
            Self { name, variants }
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum StmtNode {
    Return(Expr),
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
}

impl DefineVar {
    pub fn new(name: &String, value: Expr, ty: &TyNode) -> Self {
        Self {
            name: name.to_string(),
            value: Box::new(value),
            ty: ty.clone()
        }
    }

    pub fn wrap(self) -> Expr {
        Expr::DefVar(self)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MatchArm {
    pub pattern: Box<Expr>,
    pub body: Vec<Group2Node>,
}


#[derive(Clone, Debug, PartialEq)]
pub struct AssignVar {
    pub name: String,
    // Expr::Varなど
    pub dst: Box<Expr>,
    pub value: Box<Expr>
}

impl AssignVar {
    pub fn new(name: &String, dst: Expr, value: Expr) -> Expr {
        Expr::Assign(
            Self {
                name: name.to_string(),
                dst: Box::new(dst),
                value: Box::new(value)
            }
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
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
    LessThen((Box<Expr>, Box<Expr>)),
    GreaterThen((Box<Expr>, Box<Expr>)),
    /// `==` 値の等価比較
    Equal((Box<Expr>, Box<Expr>)),
    NotEq((Box<Expr>, Box<Expr>)),
    Match {
        pattern: Option<Box<Expr>>,
        arms: Vec<MatchArm>,
        arm_else: Option<Vec<Group2Node>>,
    },
    Loop {
        pattern: Option<Box<Expr>>,
        body: Vec<Group2Node>
    },
    InitStruct {
        name: String,
        fields: HashMap<String, Box<Expr>>
    },
    /// 列挙型のメンバへのアクセス: `Name::Mem`
    EnumVariant {
        name: String,
        variant: String,
    },
    /// 配列リテラル: `{100, 100, 100, 100}`
    Array(Vec<Expr>),
    InsertArr {
        name: String,
        dst: Box<Expr>,
        index: usize,
    },

    DefVar(DefineVar),
    Scope{
        scope: Vec<String>,
        target: Box<Expr>,
    },
    Member{
        scope: Vec<String>,
        target: Box<Expr>,
    },
}

impl Expr {
    pub fn wrap(left: Expr, right: Expr) -> (Box<Expr>, Box<Expr>) {
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
            Self::Assign(ref mut name) => return name,
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
    pub fn new() -> Self {
        Self {
            path: Vec::new()
        }
    }

    pub fn add_path(&mut self, path_name: &String) {
        self.path.push(path_name.clone());
    }

    pub fn gen_path(&self) -> String {
        // ディレクトリの最初のパスのインデックス
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
        let parent_len = self.path.len().saturating_sub(1);
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

impl Group2Node {
    pub fn change_group1(self) -> Group1Node {
        match self {
            Self::Include(v) => Group1Node::Include(v),
            Self::Line(v) => Group1Node::Line(v),
            t => panic!("{:?} <- これは対応していません", t)
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Group1Node {
    FuncDefine(FuncDefine),
    StructDefine(StructDefine),
    EnumDefine(EnumDefine),
    Include(ModPath),
    Line(String)
}


#[cfg(test)]
pub fn gen_var_node(name: &str, value: &str, ty: &str) -> Group2Node {
    Group2Node::Expr(
        Expr::DefVar(
            DefineVar {
                name: name.to_string(),
                value: Box::new(Expr::Number(value.to_string())),
                ty: TyNode::Ty(ty.to_string()),
            }
        )
    )
}



#[cfg(test)]
pub fn wrap_expr_cmp(left: &str, right: &str) -> Expr {
    Expr::LessThen(
        (
            Box::new(Expr::Number(left.to_string())),
            Box::new(Expr::Number(right.to_string())),
        )
    )
}


#[cfg(test)]
pub fn wrap_eq_expr_cmp(left: &str, right: &str) -> Expr {
    Expr::Equal(
        (
            Box::new(Expr::Number(left.to_string())),
            Box::new(Expr::Number(right.to_string())),
        )
    )
}
