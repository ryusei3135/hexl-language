
#[derive(Clone, Debug, PartialEq)]
pub enum TyNode {
    Ty(String),
    Generics((String, Box<TyNode>)),
    RefTy(String),
    TknTy(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ArgsNode {
    pub name: String,
    pub ty: TyNode,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DefGenerics {
    name: String,
    ty: TyNode,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FuncDefine {
    name: String,
    params: Vec<ArgsNode>,
    ret_ty: TyNode,
    body: Vec<Group2Node>
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
    name: String,
    value: Box<Expr>,
    generics: TyNode,
    ty: TyNode,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Number(String),
    Var(String),
    Add((Box<Expr>, Box<Expr>)),
    Sub((Box<Expr>, Box<Expr>)),
    Mul((Box<Expr>, Box<Expr>)),
    Div((Box<Expr>, Box<Expr>)),

    DefVar(DefineVar),
}

impl Expr {
    pub fn wrap(left: Expr, right: Expr) -> (Box<Expr>, Box<Expr>) {
        (Box::new(left), Box::new(right))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Group2Node {
    Stmt(StmtNode),
    Expr(Expr),
    DefVar(DefineVar),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Group1Node {
    FuncDefine(FuncDefine),
}
