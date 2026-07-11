
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
    pub name: String,
    pub params: Vec<ArgsNode>,
    pub ret_ty: TyNode,
    pub body: Vec<Group2Node>
}


impl FuncDefine {
    pub fn new(
        name: String,
        args: Vec<ArgsNode>,
        ret_ty: TyNode
    ) -> Group1Node {
        Group1Node::FuncDefine(
            Self {
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
pub struct CallInfo {
    pub name: String,
    pub args: Vec<Expr>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Group2Node {
    Stmt(StmtNode),
    Expr(Expr),
    CompleSyntax((String, Vec<String>)),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Group1Node {
    FuncDefine(FuncDefine),
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
