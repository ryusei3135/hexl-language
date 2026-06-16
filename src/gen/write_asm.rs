use super::*;
use crate::ir::inst;
use crate::AsmFormat;

pub struct Writer {
    //
}

impl Writer {
    pub fn new() -> Self {
        Self {
        }
    }

    pub fn writer(&mut self, func_tree: &FuncTree, asm_format: &AsmFormat) {
        //let func_infos: Vec<(String, FuncDefInfo)> = func_tree.gen_vec();
        let main_func = func_tree.get(&"main".to_string());

        for node in main_func.tree.iter() {
            match &node {
                inst::Inst::Expr(expr) => {
                }
                inst::Inst::Num{dst, value, size} => {
                }
                inst::Inst::Ret(idx) => {
                }
                t => panic!("{:?}", t),
            }
        }
    }

    fn write_expr_inst(&self, format: &AsmFormat, expr: &inst::ExprInst) {
        match expr.kind {
            inst::ExprKind::Add => {
            }
            inst::ExprKind::Sub => {
            }
            inst::ExprKind::Mul => {
            }
            inst::ExprKind::Div => {
            }
        }
    }
}
