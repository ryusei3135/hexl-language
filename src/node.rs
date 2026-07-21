
#[derive(Clone, Debug, PartialEq)]
pub enum TyNode {
    Ty(String),
    RefTy(String),
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
    pub body: Vec<Group2Node>
}


impl FuncDefine {
    pub fn new(
        name: String,
        args: Vec<ArgsNode>,
        ret_ty: TyNode,
        public: bool
    ) -> Group1Node {
        Group1Node::FuncDefine(
            Self {
                public,
                name: name,
                params: args,
                ret_ty: ret_ty,
                body: Vec::new(),
            }
        )
    }

    pub fn add(&mut self, node: Group2Node) {
        self.body.push(node);
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
pub enum Expr {
    Number(String),
    Str(String),
    Var(String),
    CallFunc(CallInfo),
    Assign{
        name: String,
        value: Box<Expr>,
    },
    Add((Box<Expr>, Box<Expr>)),
    Sub((Box<Expr>, Box<Expr>)),
    Mul((Box<Expr>, Box<Expr>)),
    Div((Box<Expr>, Box<Expr>)),
    LessThen((Box<Expr>, Box<Expr>)),
    GreaterThen((Box<Expr>, Box<Expr>)),
    Match {
        pattern: Option<Box<Expr>>,
        arms: Vec<MatchArm>,
        arm_else: Option<Vec<Group2Node>>,
    },
    Loop {
        pattern: Option<Box<Expr>>,
        body: Vec<Group2Node>
    },

    DefVar(DefineVar),
}

impl Expr {
    pub fn wrap(right: Expr, left: Expr) -> (Box<Expr>, Box<Expr>) {
        (Box::new(left), Box::new(right))
    }

    #[cfg(test)]
    pub fn wrap_group2(self) -> Group2Node {
        Group2Node::Expr(self)
    }
}


#[derive(Clone, Debug, PartialEq)]
pub struct ModPath {
    path: Vec<String>,
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
}

#[derive(Clone, Debug, PartialEq)]
pub struct CallInfo {
    pub name: String,
    pub args: Vec<Expr>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Group2Node {
    Stmt(StmtNode),
    Expr(Expr),
    CompleSyntax((String, Vec<String>)),
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
