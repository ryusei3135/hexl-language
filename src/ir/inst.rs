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

    pub fn get_param_reg(&self, param_regs: &Vec<usize>) -> usize {
        param_regs[self.num].clone()
    }
}


#[derive(Clone, Debug, PartialEq)]
pub struct CallFuncMetaData {
    name: String,
    params: Vec<ValueId>,
}

impl CallFuncMetaData {
    pub fn new(name: String) -> Self {
        Self {
            name,
            params: Vec::new(),
        }
    }

    pub fn insert_param(&mut self, value_id: ValueId) {
        self.params.push(value_id);
    }
}


#[derive(Clone, Debug, PartialEq)]
pub enum Inst {
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
        size: Size,
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
    pub fn gen_num(value: &String, size: &Size, dst: usize) -> Self {
        match size {
            Size::DB => {
                value.parse::<u8>().unwrap();
            }
            Size::DW => {
                value.parse::<u16>().unwrap();
            }
            Size::DD => {
                value.parse::<u32>().unwrap();
            }
            Size::DQ => {
                value.parse::<u64>().unwrap();
            }
        }
        Self::Num{
            dst,
            value: value.clone(),
            size: size.clone(),
        }
    }
}
