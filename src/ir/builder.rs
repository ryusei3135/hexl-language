use crate::{
    err,
    node
};
use std::collections::HashMap;
use std::mem;
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
                    "b1" => Self::DB,
                    "b2" => Self::DW,
                    "b4" => Self::DD,
                    "b8" => Self::DQ,
                    _ => panic!(),
                }
            }
            _ => panic!(),
        }
    }
}


#[derive(Clone, Debug, PartialEq)]
pub struct VarTree {
    pub hash: HashMap<String, usize>,
}

impl VarTree {
    pub fn new() -> Self {
        Self {
            hash: HashMap::new(),
        }
    }
    
    pub fn add(&mut self, name: &String, index: &usize) {
        self.hash.insert(name.clone(), index.clone());
    }

    pub fn get(&self, name: &String) -> usize {
        *self.hash.get(name).unwrap()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FuncDefInfo {
    pub args: Vec<node::ArgsNode>,
    pub tree: Vec<inst::Inst>,
    pub ret_ty: Option<Size>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FuncTree{
    pub func: HashMap<String, FuncDefInfo>
}

impl FuncTree {
    pub fn new() -> Self {
        Self {
            func: HashMap::new(),
        }
    }

    pub fn get(&mut self, name: &String) -> FuncDefInfo {
        self.func.remove(&String::from(name)).unwrap()
    }

    //pub fn gen_vec(&self) -> Vec<(String, FuncDefInfo)> {
      //  self.func.clone().into_iter().collect()
    //}
    
    pub fn add(
        &mut self,
        ir_tree: Vec<inst::Inst>,
        name: &String,
        args: Vec<node::ArgsNode>,
        ret_ty: Size
    ) {
        self.func.insert(
            name.clone(),
            FuncDefInfo {
                args,
                tree: ir_tree,
                ret_ty: Some(ret_ty),
            }
        );
    }
}

pub struct IR {
    pub var_tree: VarTree,
    pub func_tree: FuncTree,
    id_counter: usize,
    func_ret_ty: Option<Size>,
    ir_tree: Vec<inst::Inst>,
}


impl IR {
    pub fn new() -> Self {
        Self {
            var_tree: VarTree::new(),
            func_tree: FuncTree::new(),
            id_counter: 0,
            func_ret_ty: None,
            ir_tree: Vec::new(),
        }
    }

    pub fn builder(&mut self, nodes: &Vec<node::Group1Node>) -> Result<(), err::Errs> {
        for node in nodes {
            match node {
                node::Group1Node::FuncDefine(info) => {
                    self.func_ret_ty = Some(Size::new(&info.ret_ty));
                    self.gen_inst(&info.body.clone());
                    self.func_tree.add(
                        mem::take(&mut self.ir_tree),
                        &info.name,
                        info.params.clone(),
                        self.func_ret_ty.clone().unwrap(),
                    );
                    self.ir_tree = Vec::new();
                    self.id_counter = 0;
                }
            }
        }
        Ok(())
    }

    fn gen_inst(&mut self, node: &Vec<node::Group2Node>) {
        for stmt in node {
            println!("{:?}", stmt);
            match stmt.clone() {
                node::Group2Node::Expr(expr) => {
                    let _ = self.gen_expr_ir(expr, &Size::DD);
                }
                node::Group2Node::Stmt(stmt) => {
                    let _ = self.gen_stmt_ir(stmt);
                }
                node::Group2Node::CompleSyntax((name, nodes)) => {
                    self.ir_tree.push(
                        inst::Inst::comple{
                            name,
                            nodes,
                        }
                    );
                    self.id_counter += 1;
                }
                _ => {},
            }
        } 
    }

    /// 文のノードを生成
    fn gen_stmt_ir(&mut self, stmt: node::StmtNode) -> usize {
        let node = match stmt {
            node::StmtNode::Return(expr) => {
                let func_ret_ty = self.func_ret_ty.clone().unwrap();
                let idx = self.gen_expr_ir(expr, &func_ret_ty);
                inst::Inst::Ret(idx)
            }
        };

        self.ir_tree.push(node);
        self.id_counter += 1;
        self.id_counter
    }

    fn left_right_pair(
        &mut self,
        node: (Box<node::Expr>, Box<node::Expr>),
        expect_byte: &Size
    ) -> (usize, usize) {
        (
            self.gen_expr_ir(*node.0, &expect_byte),
            self.gen_expr_ir(*node.1, &expect_byte)
        )
    }

    fn gen_expr_ir(&mut self, expr: node::Expr, expect_byte: &Size) -> usize {
        let inst = match expr {
            node::Expr::Add(node) => {
                let pair = self.left_right_pair(node, &expect_byte);
                inst::ExprInst {
                    dst: self.id_counter,
                    ls: pair.0,
                    rs: pair.1,
                    kind: inst::ExprKind::Add,
                }.new()
            }
            node::Expr::Sub(node) => {
                let pair = self.left_right_pair(node, &expect_byte);
                inst::ExprInst {
                    dst: self.id_counter,
                    ls: pair.0,
                    rs: pair.1,
                    kind: inst::ExprKind::Sub,
                }.new()
            }
            node::Expr::Mul(node) => {
                let pair = self.left_right_pair(node, &expect_byte);
                inst::ExprInst {
                    dst: self.id_counter,
                    ls: pair.0,
                    rs: pair.1,
                    kind: inst::ExprKind::Mul,
                }.new()
            }
            node::Expr::Div(node) => {
                let pair = self.left_right_pair(node, &expect_byte);
                inst::ExprInst {
                    dst: self.id_counter,
                    ls: pair.0,
                    rs: pair.1,
                    kind: inst::ExprKind::Div,
                }.new()
            }
            node::Expr::Number(value) => {
                inst::Inst::gen_num(&value, &expect_byte, self.id_counter)
            }
            node::Expr::Str(value) => {
                inst::Inst::Str{
                    dst: self.id_counter,
                    value,
                }
            } 
            node::Expr::DefVar(mut var) => {
                let value_idx = self.gen_expr_ir(
                    *var.value,
                    &Size::new(&var.ty)
                );
                self.var_tree.add(&var.name, &value_idx);
                inst::Inst::Mov{
                    name: Some(mem::take(&mut var.name)),
                    dst: self.id_counter,
                    src: value_idx,
                }
            }
            node::Expr::CallFunc(func) => {
                let info = self.func_tree.get(&func.name);
                let mut arg_len = info.args.len();
                let mut params = Vec::<usize>::new();
                let def_args = info.args.clone();

                while arg_len > 0 {
                    let expr_arg = func.args.get(arg_len - 1).unwrap().clone();
                    let ty = Size::new(&def_args[arg_len - 1].ty).clone();
                    let idx = self.gen_expr_ir(expr_arg, &ty);
                    params.push(idx);
                    arg_len -= 1;
                }
                inst::Inst::CallFunc {
                    name: func.name.clone(),
                    args: params,
                }
            }
            node::Expr::Var(name) => {
                return self.var_tree.get(&name);
            }
            t => panic!("{:?}", t),
        };

        self.ir_tree.push(inst);

        self.id_counter += 1;
        self.id_counter - 1
    }
} 
