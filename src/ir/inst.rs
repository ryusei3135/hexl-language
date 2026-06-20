use super::*;


#[derive(Clone, Debug, PartialEq)]
pub enum ExprKind {
    Add,
    Sub,
    Mul,
    Div,
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
pub enum Inst {
    Expr(ExprInst),
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
    CallFunc {
        name: String,
        args: Vec<ValueId>,
    },
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
