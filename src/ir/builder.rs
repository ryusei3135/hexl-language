use crate::{
    err,
    node
};
use std::collections::HashMap;
use std::{
    mem,
};
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
                    "short" => Self::DW,
                    "int" => Self::DD,
                    "long" => Self::DQ,
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

#[derive(Clone, Debug, PartialEq)]
pub struct VarTree {
    pub hash: HashMap<String, VarType>,
}

impl VarTree {
    pub fn new() -> Self {
        Self {
            hash: HashMap::new(),
        }
    }
    
    /// ## K
    /// - `usize`の場合 local変数
    /// - 'l' = local
    /// - 'p' = param
    pub fn push<const K: char>(&mut self, name: &String, index: &usize) {
        let var = match K {
            'l' => {
                VarType::Local(*index)
            }
            'p' => {
                VarType::Param(*index)
            }
            _ => panic!("system err VarTree::AddのKには、`l`か`p`以外入れられません"),
        };
        self.hash.insert(name.clone(), var);
    }

    pub fn get(&self, name: &String) -> &VarType {
        self.hash.get(name).expect(name)
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
        self.func.get(name).unwrap().clone()
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
    func_ret_ty: Option<Size>,
    ir_tree: Vec<inst::Inst>,
    pattern_labels: usize,
    jmp_labels: usize,
}


impl IR {
    pub fn new() -> Self {
        Self {
            var_tree: VarTree::new(),
            func_tree: FuncTree::new(),
            id_counter: 0,
            func_ret_ty: None,
            ir_tree: Vec::new(),
            pattern_labels: 0,
            jmp_labels: 0,
        }
    }

    pub fn builder(&mut self, nodes: &Vec<node::Group1Node>) -> Result<(), err::Errs> {
        for node in nodes {
            match node {
                node::Group1Node::FuncDefine(info) => {
                    self.func_ret_ty = Some(Size::new(&info.ret_ty));
                    self.register_argument(&info.params);
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
                _ => {}
            }
        }
        Ok(())
    }

    fn gen_inst(&mut self, node: &Vec<node::Group2Node>) -> usize {
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
                t => println!("{:?}", t),
            }
        } 
        self.id_counter
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
            node::Expr::Assign { name, value } => {
                let right_expr_idx = self.gen_expr_ir(*value, &expect_byte);

                inst::Inst::AssignVar {
                    name,
                    value: right_expr_idx,
                }
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
                self.var_tree.push::<'l'>(&var.name, &value_idx);
                inst::Inst::Mov{
                    name: Some(mem::take(&mut var.name)),
                    dst: self.id_counter,
                    src: value_idx,
                }
            }
            node::Expr::CallFunc(meta_data) => {
                // 関数の定義を取得
                let defined_func_data = self.func_tree.get(&meta_data.name);
                let def_args = defined_func_data.args.clone();
                // 関数のノードを作成
                let mut func_meta_data 
                    = inst::CallFuncMetaData::new(meta_data.name);

                for (index, _) in meta_data.args.iter().enumerate() {
                    let expr_arg = meta_data.args.get(index).unwrap().clone();
                    let ty = Size::new(&def_args[index].ty).clone();
                    let idx = self.gen_expr_ir(expr_arg, &ty);
                    func_meta_data.insert_param_parent_id(idx);
                }
                self.id_counter += 1;
                inst::Inst::CallFunc(func_meta_data)
            }
            node::Expr::Var(name) => {
                return match self.var_tree.get(&name) {
                    VarType::Local(index) => {
                        *index
                    }
                    VarType::Param(param) => {
                        // 引数のノード
                        *param
                    }
                };
            }
            node::Expr::Match{pattern, arms, arm_else} => {
                return self.gen_match_expr_ir(&pattern, &arms, &arm_else);
            }
            node::Expr::Loop { pattern, body } => {
                return self.gen_loop_expr_ir(pattern, &body);
            }
        };

        self.ir_tree.push(inst);

        self.id_counter += 1;
        self.id_counter - 1
    }

    #[inline(always)]
    fn gen_loop_expr_ir(
        &mut self,
        pattern: Option<Box<node::Expr>>,
        body: &Vec<node::Group2Node>,
    ) -> usize {
        // 反復処理が始まる場所を作成
        let start = self.pattern_labels;
        self.pattern_labels += 1;
        let end = self.pattern_labels;
        self.pattern_labels += 1;
        crate::push_jmp_code!(self, Block, start.clone());

        // ループする条件の作成
        if let Some(expr) = pattern {
            crate::push_jmp_code!(self, ExpectJmp, &self.pattern_labels);
            let _ = self.gen_expr_ir(*expr, &Size::DD);
            // もし条件がfalseならendまでジャンプ
            crate::push_jmp_code!(self, Jmp, &end);
            // 条件がtrueのときジャンプする場所
            crate::push_jmp_code!(self, Block, &self.pattern_labels);
        }
        self.gen_inst(&body);
        crate::push_jmp_code!(self, Jmp, &start);
        crate::push_jmp_code!(self, Block, &end);
        self.id_counter - 1
    }

    #[inline(always)]
    fn gen_match_expr_ir(
        &mut self,
        pattern: &Option<Box<node::Expr>>,
        arms: &Vec<node::MatchArm>,
        arm_else: &Option<Vec<node::Group2Node>>
    ) -> usize {
        if let Some(_expr) = pattern {
            //
        }

        // 条件分岐の塊を作成
        let end_label = if arms.len() == 1 {
            // アセンブリコードを生成するときに作成するラベル
            self.jmp_labels = self.pattern_labels + 1;
            crate::push_jmp_code!(self, ExpectJmp, self.jmp_labels);
            // 条件が一つだけの場合]
            // 条件式の生成
            self.gen_expr_ir(*arms[0].pattern.clone(), &Size::DD);
            //self.push_jmp_code(self.pattern_labels);
            self.jmp_labels.clone() + 1
        } else {
            // 条件が複数の場合
            for arm in arms.clone() {
                crate::push_jmp_code!(self, ExpectJmp, self.pattern_labels);
                self.gen_expr_ir(*arm.pattern, &Size::DD);
            }
            arms.len() + 1
        };

        // elseの処理を作成
        if let Some(arm) = arm_else.clone() {
            self.gen_inst(&arm);
            crate::push_jmp_code!(self, Jmp, &end_label);
        }
        // 処理内容の作成
        for arm in arms {
            // 自分のラベルを作成
            crate::push_jmp_code!(self, Block, self.jmp_labels);
            self.gen_inst(&arm.body);
        }
        // 条件が終了したときにジャンプする場所を指定
        crate::push_jmp_code!(self, Block, end_label);
        return self.id_counter - 1;
    }

    /// 関数のノードを生成するときに、引数を登録
    fn register_argument(&mut self, params: &Vec<node::ArgsNode>) {
        for (index, param) in params.iter().enumerate() {
            self.var_tree.push::<'p'>(
                &param.name,
                &index,
            );
            self.ir_tree.push(
                inst::Inst::Param(
                    inst::ParamMetaData::new(
                        param.name.to_string(),
                        index,
                        self.ir_tree.len()
                    )
                )
            );
            self.id_counter += 1;
        }
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

    #[test]
    fn check_argument() {
        let mut list = InstCheckList::new();
        list.gen(
            "
            add(a: b1): b1 {
                a = 10
                ret a
            }
            "
        );
        let body = list.ir.func_tree.get(&"add".to_string()).body;
        assert_eq!(
            body,
            vec![
                ir::inst::Inst::Param(
                    ir::inst::ParamMetaData::new("a".to_string(), 0, 0)
                ),
                ir::inst::Inst::Num { dst: 1, value: "10".to_string(), size: Size::DD },
                ir::inst::Inst::AssignVar { name: "a".to_string(), value: 1 },
                ir::inst::Inst::Ret(0),
            ]
        );
    }

    #[test]
    pub fn check_loop_inst() {
        let mut list = InstCheckList::new();
        list.gen(
            "
                main(): b1 {
                    loop 1 < 10 {
                        b: b4 = 10
                        b = b + 6
                    }
                    ret 1
                }
            "
        );
        let body = list.ir.func_tree.get(&"main".to_string()).body;
        println!("{:?}", body);
        assert_eq!(
            body[0..=6],
            vec![
                ir::inst::Inst::Block("L0".to_string()),
                ir::inst::Inst::ExpectJmp("L2".to_string()),
                ir::inst::Inst::gen_num("10", &Size::DD, 2),
                ir::inst::Inst::gen_num("1", &Size::DD, 3),
                ir::inst::Inst::Expr(
                    ir::inst::ExprInst{dst: 4, ls: 2, rs: 3, kind: ir::inst::ExprKind::LessThen}
                ),
                ir::inst::Inst::Jmp("L1".to_string()),
                ir::inst::Inst::Block("L2".to_string()),
            ]
        );
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
        println!("{:?}", body);
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
