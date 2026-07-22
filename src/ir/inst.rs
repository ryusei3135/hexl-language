use super::*;


#[derive(Clone, Debug, PartialEq)]
pub enum ExprKind {
    Add,
    Sub,
    Mul,
    Div,
    LessThen,
    GreaterThen,
}

pub type ValueId = usize;

#[derive(Clone, Debug, PartialEq)]
pub struct ExprInst {
    pub dst: ValueId,
    pub ls: ValueId,
    pub rs: ValueId,
    pub kind: ExprKind,
}

impl ExprInst {
    pub fn new(
        self
    ) -> Inst {
        Inst::Expr(self)
    }
}


#[derive(Clone, Debug, PartialEq)]
pub struct ParamMetaData {
    pub name: String,
    pub num: usize,  // 何番目の引数か？
    pub dst: usize,
}

impl ParamMetaData {
    pub fn new(name: String, num: usize, dst: usize) -> Self {
        Self {
            name,
            num,
            dst
        }
    }
}


#[derive(Clone, Debug, PartialEq)]
pub struct CallFuncMetaData {
    pub path: Vec<String>,
    pub public: bool,
    pub name: String,
    pub params: Vec<ValueId>,
}

impl CallFuncMetaData {
    pub fn new(name: String) -> Self {
        Self {
            path: Vec::new(),
            public: false,
            name,
            params: Vec::new(),
        }
    }

    pub fn insert_param_parent_id(&mut self, value_id: ValueId) {
        self.params.push(value_id);
    }

    pub fn module(&mut self, path: &Vec<String>) {
        self.path = path.clone();
    }
}


#[derive(Clone, Debug, PartialEq)]
pub enum Inst {
    ExternFunc(String),
    Expr(ExprInst),
    Block(String),
    Jmp(String),
    ExpectJmp(String),// ジャンプする場所
    Mov{
        name: Option<String>,
        dst: ValueId,
        src: ValueId,
    },
    Num{
        dst: ValueId,
        value: String,
        size: types::Size,
    },
    Str {
        dst: ValueId,
        value: String,
    },
    CallFunc(CallFuncMetaData),
    Comple {
        name: String,
        nodes: Vec<String>,
    },
    AssignVar{
        name: String,
        value: usize,
    },
    Param(ParamMetaData),
    Ret(ValueId),
}

impl Inst {
    pub fn gen_num(value: &str, size: &types::Size, dst: usize) -> Self {
        match size {
            types::Size::DB => {
                value.parse::<u8>().unwrap();
            }
            types::Size::DW => {
                value.parse::<u16>().unwrap();
            }
            types::Size::DD => {
                value.parse::<u32>().unwrap();
            }
            types::Size::DQ => {
                value.parse::<u64>().unwrap();
            }
        }
        Self::Num{
            dst,
            value: value.to_string(),
            size: size.clone(),
        }
    }
}
