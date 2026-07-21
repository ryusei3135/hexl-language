use super::*;


#[derive(Clone, Debug, PartialEq)]
pub enum Size {
    DB,
    DW,
    DD,
    DQ
}

impl Size {
    pub fn new(ty: &node::TyNode) -> Self {
        match ty {
            node::TyNode::Ty(ty) => {
                match ty.as_str() {
                    "char" => Self::DB,
                    "u16" => Self::DW,
                    "int" => Self::DD,
                    "u64" => Self::DQ,
                    _ => panic!(),
                }
            }
            _ => panic!(),
        }
    }
}


#[derive(Clone, Debug, PartialEq)]
pub enum VarType {
    Local(usize),// 変数の値や式のidx
    Param(usize),//これは、左から何番目の引数かを保存
}
