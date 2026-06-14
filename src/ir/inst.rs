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
        dst: ValueId,
        src: ValueId,
    },
    Num{
        dst: ValueId,
        value: Vec<u8>,
    },
    CallFunc {
        name: String,
        args: Vec<ValueId>,
    },
    Ret(ValueId),
}

impl Inst {
    pub fn gen_num(value: &String, size: &Size, dst: usize) -> Self {
        let bytes = if let Ok(num) = value.parse::<usize>() {
            let mut bytes = num.to_le_bytes().to_vec();
            bytes.resize(size.size(), 0);
            bytes
        } else {
            panic!();
        };
        Self::Num{
            dst,
            value: bytes
        }
    }
}
