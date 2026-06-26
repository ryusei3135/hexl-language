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
    pub body: Vec<inst::Inst>,
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
        body: Vec<inst::Inst>,
        name: &String,
        args: Vec<node::ArgsNode>,
        ret_ty: Size
    ) {
        self.func.insert(
            name.clone(),
            FuncDefInfo {
                args,
                body,
                ret_ty: Some(ret_ty),
            }
        );
    }
}

pub struct IR {
    pub var_tree: VarTree,
    pub func_tree: FuncTree,
    id_counter: usize,
    jmp_label_counter: usize,
    func_ret_ty: Option<Size>,
    ir_tree: Vec<inst::Inst>,
    branch_top: usize,
}


impl IR {
    pub fn new() -> Self {
        Self {
            var_tree: VarTree::new(),
            func_tree: FuncTree::new(),
            id_counter: 0,
            jmp_label_counter: 0,
            func_ret_ty: None,
            ir_tree: Vec::new(),
            branch_top: 0,
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
            match stmt.clone() {
                node::Group2Node::Expr(expr) => {
                    let _ = self.gen_expr_ir(expr, &Size::DD);
                }
                node::Group2Node::Stmt(stmt) => {
                    let _ = self.gen_stmt_ir(stmt);
                }
                node::Group2Node::CompleSyntax((name, nodes)) => {
                    self.ir_tree.push(
                        inst::Inst::Comple{
                            name,
                            nodes,
                        }
                    );
                    self.id_counter += 1;
                }
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

    #[inline(always)]
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

    /// 指定した種類の式のノードを作成する
    #[inline(always)]
    fn build_expr_inst(
        &mut self,
        node: (Box<node::Expr>, Box<node::Expr>),
        expect_byte: &Size,
        kind: inst::ExprKind,
    ) -> inst::Inst {
        let pair = self.left_right_pair(node, &expect_byte);
        inst::ExprInst {
            dst: self.id_counter,
            ls: pair.0,
            rs: pair.1,
            kind,
        }.new()
    }

    fn gen_expr_ir(&mut self, expr: node::Expr, expect_byte: &Size) -> usize {
        let inst = match expr {
            node::Expr::Add(node) => {
                self.build_expr_inst(node, &expect_byte, inst::ExprKind::Add)
            }
            node::Expr::Sub(node) => {
                self.build_expr_inst(node, &expect_byte, inst::ExprKind::Sub)
            }
            node::Expr::Mul(node) => {
                self.build_expr_inst(node, &expect_byte, inst::ExprKind::Mul)
            }
            node::Expr::Div(node) => {
                self.build_expr_inst(node, &expect_byte, inst::ExprKind::Div)
            }
            node::Expr::LessThen(node) => {
                self.build_expr_inst(node, &expect_byte, inst::ExprKind::LessThen)
            }
            node::Expr::GreaterThen(node) => {
                self.build_expr_inst(node, &expect_byte, inst::ExprKind::GreaterThen)
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
            node::Expr::Match{pattern, arms, arm_else} => {
                let mut pattern_labels = 0;
                let mut jmp_labels = 0;

                // 条件の式を生成
                if arms.len() == 1 {
                    // 条件が一つだけの場合
                    self.gen_expr_ir(*arms[0].pattern.clone(), &Size::DD);
                    pattern_labels += 1;
                    self.ir_tree.push(inst::Inst::ExpectJmp(format!("L{}", pattern_labels)));
                    self.id_counter += 1;
                } else {
                    // 条件が複数の場合
                    for arm in arms.clone() {
                        self.gen_expr_ir(*arm.pattern, &Size::DD);
                        self.ir_tree.push(inst::Inst::Jmp(format!("L{}", pattern_labels)));
                        self.id_counter += 1;
                        pattern_labels += 1;
                    }
                }

                // elseの処理を作成
                pattern_labels += 1;
                if let Some(arm) = arm_else.clone() {
                    self.gen_inst(&arm);
                    self.ir_tree.push(inst::Inst::Jmp(format!("L{}", pattern_labels)));
                    self.id_counter += 1;
                    jmp_labels += 1;
                }
                // 処理内容の作成
                for arm in arms {
                    // 自分のラベルを作成
                    self.ir_tree.push(inst::Inst::Block(format!("L{}", jmp_labels)));
                    jmp_labels += 1;
                    self.id_counter += 1;
                    self.gen_inst(&arm.body);
                    // 自分の処理が終了したときに処理の最後の場所のラベルまでｊｍｐする
                    self.ir_tree.push(inst::Inst::Jmp(format!("L{}", pattern_labels)));
                    self.id_counter += 1;
                }
                // 条件が終了したときにジャンプする場所を指定
                self.ir_tree.push(inst::Inst::Block(format!("L{}", pattern_labels)));
                self.id_counter += 1;
                return self.id_counter - 1;
            }
        };

        self.ir_tree.push(inst);

        self.id_counter += 1;
        self.id_counter - 1
    }
} 


#[cfg(test)]
mod tests {
    use crate::{lex, parse, ir};
    use super::*;

    struct InstCheckList {
        lex: lex::Lexer,
        par: parse::Parser,
        pub ir: ir::IR,
    }

    impl InstCheckList {
        pub fn new() -> Self {
            Self {
                lex: lex::Lexer::new(),
                par: parse::Parser::new(),
                ir: ir::IR::new(),
            }
        }

        pub fn gen(&mut self, contents: &str) {
            self.lex.analy(&contents.to_string());
            let nodes = self.par
                .parser(self.lex.gen_tkns.clone())
                .map_err(|v| v.print_log(&contents.to_string()))
                .unwrap();
            self.ir.builder(&nodes).unwrap();
        }
    }

    /// match式の後に処理が続いているか確認
    #[test]
    pub fn check_match_inst() {
        let mut list = InstCheckList::new();
        list.gen(
            "
                main(): b1 {
                    a: b1 = 0
                    match {
                        a < 10 {
                            b: b1 = 5
                        }
                        | {
                            b: b1 = 10
                        }
                    }
                    c: b1 = 20
                }
            "
        );
        let body = list.ir.func_tree.get(&"main".to_string()).body;
        assert_eq!(
            body.last().unwrap(),
            &ir::inst::Inst::Mov {
                name: Some("c".to_string()),
                dst: body.len() - 1,
                src: body.len() - 2,
            }
        );
    }
} 
