use crate::{
    err,
    node
};
use super::*;



#[derive(Clone, Debug, PartialEq)]
pub struct VarTree {
    pub hash: HashMap<String, types::VarType>,
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
                types::VarType::Local(*index)
            }
            'p' => {
                types::VarType::Param(*index)
            }
            _ => panic!("system err VarTree::AddのKには、`l`か`p`以外入れられません"),
        };
        self.hash.insert(name.clone(), var);
    }

    pub fn get(&self, name: &String) -> &types::VarType {
        self.hash.get(name).expect(name)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FuncDefInfo {
    pub module: Option<String>,
    pub args: Vec<node::ArgsNode>,
    pub body: Vec<inst::Inst>,
    pub ret_ty: Option<node::TyNode>,
    pub public: bool,
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

    pub fn get(&mut self, name: &String, module_name: Option<&String>) -> Option<FuncDefInfo> {
        if let Some(func) = self.func.get(name) {
            // 指定された関数がモジュールに入っていないかつ、自分も
            // モジュールを指定していない場合そのまま関数のデータを返す
            if module_name.is_none() && func.module.is_none() {
                return Some(func.clone());
            }

            if func.module == module_name.map(|v| v.to_string()) {
                Some(func.clone())
            } else {
                panic!("not found this module in {}", name);
            }
        } else {
            None
        }
    }
    
    pub fn add(
        &mut self,
        body: Vec<inst::Inst>,
        meta_data: &node::FuncDefine,
        ret_ty: &node::TyNode,
    ) {
        self.func.insert(
            meta_data.name.clone(),
            FuncDefInfo {
                module: None,
                args: meta_data.params.clone(),
                body,
                ret_ty: Some(ret_ty.clone()),
                public: meta_data.public,
            }
        );
    }
}


#[derive(Clone, Debug)]
pub struct FuncDefMetaData {
    module: Option<String>,
    name: String,
    params: Vec<node::ArgsNode>,
    ret_ty: Option<node::TyNode>,
}

impl FuncDefMetaData {
    /// moduleは自分自身がどのモジュールに属しているか
    /// Noneの場合は、#includeで関数の名前ごと指定しているか
    /// 自分のファイルの中にあるかのどちらか
    pub fn new(
        info: &node::FuncDefine,
        module: Option<&String>
    ) -> Self {
        Self {
            module: module.map(|v| v.clone()),
            name: info.name.clone(),
            params: info.params.clone(),
            ret_ty: Some(info.ret_ty.clone()),
        }
    }

    pub fn add_self_module_name(&mut self, self_name: &String) {
        self.module = Some(self_name.to_string());
    }

    pub fn gen(&self) -> FuncDefInfo {
        FuncDefInfo {
            module: None,
            args: self.params.clone(),
            body: Vec::new(),
            ret_ty: self.ret_ty.clone(),
            public: true,
        }
    }
}


pub struct IR {
    pub var_tree: VarTree,
    pub extern_funcs: Vec<inst::Inst>,
    id_counter: usize,
    func_ret_ty: Option<node::TyNode>,
    ir_tree: Vec<inst::Inst>,
    pattern_labels: usize,
    jmp_labels: usize,
    // 関数の情報
    pub func_tree: FuncTree,
    // 外部の関数の情報
    extern_func_tree: Vec<FuncDefMetaData>,
    // 自身が公開する関数の配列:
    pub public_func_tree: Vec<String>,
    define_meta_data: Vec<FuncDefMetaData>,
    // 定義済みの構造体の情報
    pub struct_tree: HashMap<String, node::StructDefine>,
    // 定義済みの列挙型の情報
    pub enum_tree: HashMap<String, node::EnumDefine>,
}


impl IR {
    pub fn new() -> Self {
        Self {
            var_tree: VarTree::new(),
            extern_funcs: Vec::new(),
            id_counter: 0,
            func_ret_ty: None,
            ir_tree: Vec::new(),
            pattern_labels: 0,
            jmp_labels: 0,
            // 関数の情報を初期化
            func_tree: FuncTree::new(),
            extern_func_tree: Vec::new(),
            public_func_tree: Vec::new(),
            define_meta_data: Vec::new(),
            struct_tree: HashMap::new(),
            enum_tree: HashMap::new(),
        }
    }

    pub fn builder(
        &mut self,
        nodes: &Vec<node::Group1Node>,
        #[cfg(not(test))] settings: &crate::cmd_line_args::OptSettings
    ) -> Result<Vec<FuncDefMetaData>, err::ErrKind> {

        for node in nodes {
            match node {
                node::Group1Node::FuncDefine(info) => {
                    // 関数の情報を登録
                    self.entry_func_info(&info);

                    self.register_argument(&info.params);
                    self.gen_inst(&info.body.clone());

                    // 関数の処理内容をpush
                    self.push_func_ir_tree(&info);
                    // 使うデータを初期化
                    self.ir_tree = Vec::new();
                    self.id_counter = 0;
                }
                #[cfg(not(test))]
                node::Group1Node::Include(path) => {
                    // パスの情報を作成
                    let dir = path.gen_path();
                    let new_setting = settings
                        .new_file(&dir);
                    let mut extern_fn_tree: Vec<FuncDefMetaData> = crate::build(&new_setting).unwrap();
                    // 関数にモジュールの名前を追加
                    extern_fn_tree
                        .iter_mut()
                        .for_each(
                            |v| {
                                v.add_self_module_name(
                                    path
                                    .path
                                    .last()
                                    .unwrap()
                                )
                            }
                        );
                    self.make_extern_func_inst(&extern_fn_tree);
                    self.extern_func_tree.extend(extern_fn_tree);
                }
                node::Group1Node::StructDefine(info) => {
                    // 構造体の情報を登録
                    self.struct_tree.insert(info.name.clone(), info.clone());
                }
                node::Group1Node::EnumDefine(info) => {
                    // 列挙型の情報を登録
                    self.enum_tree.insert(info.name.clone(), info.clone());
                }
                _ => {}
            }
        }
        Ok(mem::take(&mut self.define_meta_data))
    }
    
    /// 関数の情報を関数ツリーに登録
    fn push_func_ir_tree(&mut self, info: &node::FuncDefine) {
        // 関数のデータをpush
        self.func_tree.add(
            // 関数の処理
            mem::take(&mut self.ir_tree),
            // 関数の引数や名前など
            &info,
            // 関数の戻り値
            &info.ret_ty,
        );
    }

    /// 現在処理中の関数の情報を登録する
    /// **これは自分自身のファイルの中の関数**
    fn entry_func_info(&mut self, info: &node::FuncDefine) {
        self.func_ret_ty = Some(info.ret_ty.clone());
        self.define_meta_data.push(FuncDefMetaData::new(&info, None));
        // 公開する関数を登録   
        if info.public {
            self.public_func_tree.push(info.name.to_string());
        }
    }

    /// 外部の関数を定義するノードを
    /// 作成し、スタックする関数
    /// アセンブリ言語を出力する際にだけ使う
    fn make_extern_func_inst(&mut self, fn_tree: &Vec<FuncDefMetaData>) {
        for func in fn_tree {
            self.extern_funcs.push(
                inst::Inst::ExternFunc(func.name.clone())
            );
        }
    }

    fn gen_inst(&mut self, node: &Vec<node::Group2Node>) -> usize {
        for stmt in node {
            match stmt.clone() {
                node::Group2Node::Expr(expr) => {
                    let _ = self.gen_expr_ir(expr, &types::Size::DD);
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
                node::Group2Node::Scope { scope, target } => {
                    if let node::Expr::CallFunc(call_func_node) = *target {
                        self.gen_call_func_ir(scope.last(), &call_func_node);
                    } else {
                        panic!();
                    }
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
                let func_ret_ty = self.func_ret_ty
                    .as_ref()
                    .unwrap();
                let idx = self.gen_expr_ir(expr, &types::Size::new(&func_ret_ty));
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
        expect_byte: &types::Size
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
        expect_byte: &types::Size,
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

    fn gen_call_func_ir(
        &mut self,
        module_name: Option<&String>,
        meta_data: &node::CallInfo
    ) -> inst::Inst {
        // 関数の定義を取得
        let defined_func_data = {
            if let Some(def_data) = self.func_tree.get(&meta_data.name, module_name) {
                def_data
            } else {
                let result = self.extern_func_tree
                    .iter()
                    .find(|v| v.name.as_str() == &meta_data.name);
                    if let Some(def_data) = result {
                        def_data.gen()
                    } else {
                        panic!();
                    }
                }
            };
        let def_args = defined_func_data.args.clone();
        // 関数のノードを作成
        let mut func_meta_data 
            = inst::CallFuncMetaData::new(meta_data.name.clone());

        for (index, _) in meta_data.args.iter().enumerate() {
            let expr_arg = meta_data.args.get(index).unwrap().clone();
            let ty = types::Size::new(&def_args[index].ty).clone();
            let idx = self.gen_expr_ir(expr_arg, &ty);
            println!(">>: {:?}", idx);
            func_meta_data.insert_param_parent_id(idx);
        }
        inst::Inst::CallFunc(func_meta_data)
    }


    fn gen_expr_ir(&mut self, expr: node::Expr, expect_byte: &types::Size) -> usize {
        let inst = match expr {
            node::Expr::Add(node) => {
                self.build_expr_inst(node, &expect_byte, inst::ExprKind::Sub)
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
                    &types::Size::new(&var.ty)
                );
                self.var_tree.push::<'l'>(&var.name, &value_idx);
                inst::Inst::Mov{
                    name: Some(mem::take(&mut var.name)),
                    dst: self.id_counter,
                    src: value_idx,
                }
            }
            node::Expr::CallFunc(meta_data) => {
                self.gen_call_func_ir(None, &meta_data)
            }
            node::Expr::Var(name) => {
                return match self.var_tree.get(&name) {
                    types::VarType::Local(index) => {
                        *index
                    }
                    types::VarType::Param(param) => {
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
            let _ = self.gen_expr_ir(*expr, &types::Size::DD);
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
            self.gen_expr_ir(*arms[0].pattern.clone(), &types::Size::DD);
            //self.push_jmp_code(self.pattern_labels);
            self.jmp_labels.clone() + 1
        } else {
            // 条件が複数の場合
            for arm in arms.clone() {
                crate::push_jmp_code!(self, ExpectJmp, self.pattern_labels);
                self.gen_expr_ir(*arm.pattern, &types::Size::DD);
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
        let body = list.ir.func_tree.get(&"add".to_string(), None).unwrap().body;
        assert_eq!(
            body,
            vec![
                ir::inst::Inst::Param(
                    ir::inst::ParamMetaData::new("a".to_string(), 0, 0)
                ),
                ir::inst::Inst::Num { dst: 1, value: "10".to_string(), size: types::Size::DD },
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
        let body = list.ir.func_tree.get(&"main".to_string(), None).unwrap().body;
        println!("{:?}", body);
        assert_eq!(
            body[0..=6],
            vec![
                ir::inst::Inst::Block("L0".to_string()),
                ir::inst::Inst::ExpectJmp("L2".to_string()),
                ir::inst::Inst::gen_num("10", &types::Size::DD, 2),
                ir::inst::Inst::gen_num("1", &types::Size::DD, 3),
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
        let body = list.ir.func_tree.get(&"main".to_string(), None).unwrap().body;
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
