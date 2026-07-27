
use super::*;



pub struct IR {
    pub var_tree: def_tree::VarTree,
    pub extern_funcs: Vec<inst::Inst>,
    id_counter: usize,
    func_ret_ty: Option<node::TyNode>,
    ir_tree: Vec<inst::Inst>,
    pattern_labels: usize,
    jmp_labels: usize,
    // 関数の情報
    pub func_tree: def_tree::FuncTree,
    // 外部の関数の情報
    extern_func_tree: Vec<def_tree::FuncDefMetaData>,
    // 自身が公開する関数の配列:
    pub public_func_tree: Vec<String>,
    define_meta_data: Vec<def_tree::FuncDefMetaData>,
    // 定義済みの構造体の情報
    pub struct_tree: def_tree::StructTree,
    // 定義済みの列挙型の情報
    pub enum_tree: HashMap<String, node::EnumDefine>,
}


impl IR {
    pub fn new() -> Self {
        Self {
            var_tree: def_tree::VarTree::new(),
            extern_funcs: Vec::new(),
            id_counter: 0,
            func_ret_ty: None,
            ir_tree: Vec::new(),
            pattern_labels: 0,
            jmp_labels: 0,
            // 関数の情報を初期化
            func_tree: def_tree::FuncTree::new(),
            extern_func_tree: Vec::new(),
            public_func_tree: Vec::new(),
            define_meta_data: Vec::new(),
            struct_tree: def_tree::StructTree::new(),
            enum_tree: HashMap::new(),
        }
    }

    pub fn builder(
        &mut self,
        nodes: &Vec<node::Group1Node>,
        #[cfg(not(test))] settings: &crate::cmd_line_args::OptSettings
    ) -> Result<Vec<def_tree::FuncDefMetaData>, err::ErrKind> {

        // 構造体・列挙型は、定義された場所より前で使われる場合があるので
        // 先に全て登録しておく(前方参照に対応するため)
        for node in nodes {
            match node {
                // 構造体を登録
                node::Group1Node::StructDefine(info) => {
                    self.struct_tree.add(info);
                }
                node::Group1Node::EnumDefine(info) => {
                    self.enum_tree.insert(info.name.clone(), info.clone());
                }
                _ => {}
            }
        }

        for node in nodes {
            match node {
                node::Group1Node::FuncDefine(info) => {
                    // 関数の情報を登録
                    self.entry_func_info(&info);

                    self.push_param_meta_data(&info.params);
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
                    let mut extern_fn_tree: Vec<def_tree::FuncDefMetaData> = crate::build(&new_setting).unwrap();
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
                    self.struct_tree.add(info);
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
    
    /// 型からサイズを求める
    ///
    /// 組み込み型(`byte`/`u16`/`int`/`u64`)は`types::Size::new`と
    /// 同じ結果を返すが、構造体・列挙型などのユーザー定義の型名が渡された
    /// 場合は`struct_tree`/`enum_tree`を参照して解決する
    fn size_of(&self, ty: &node::TyNode) -> types::Size {
        match ty {
            node::TyNode::Ty(name) => {
                if types::Size::is_builtin_ty_name(name) {
                    return types::Size::new(ty);
                }

                if self.enum_tree.contains_key(name) {
                    // 列挙型は現在、タグ(整数値)として扱う
                    return types::Size::DD;
                }

                if let Some(struct_def) = self.struct_tree.get(&name) {
                    let fields = struct_def
                        .fields
                        .iter()
                        .map(|field| {
                            Box::new((field.name.clone(), self.size_of(&field.ty)))
                        })
                        .collect();
                    return types::Size::Struct(fields);
                }

                panic!("未定義の型です: {}", name);
            }
            node::TyNode::Pointer { is_const, ty_name } => {
                types::Size::Pointer{
                    ty: Box::new(self.size_of(ty_name)),
                    is_const: is_const.clone()
                }
            }
            // スタック/静的領域の型は、要素の型と同じサイズを持つ
            node::TyNode::Stack { name, .. } | node::TyNode::Static { name, .. } => {
                self.size_of(&node::TyNode::Ty(name.clone()))
            }
            node::TyNode::RefTy(inner) => self.size_of(inner),
        }
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
        self.define_meta_data.push(def_tree::FuncDefMetaData::new(&info, None));
        // 公開する関数を登録   
        if info.public {
            self.public_func_tree.push(info.name.to_string());
        }
    }

    /// 外部の関数を定義するノードを
    /// 作成し、スタックする関数
    /// アセンブリ言語を出力する際にだけ使う
    fn make_extern_func_inst(&mut self, fn_tree: &Vec<def_tree::FuncDefMetaData>) {
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
                t => println!("gen inst {:?}", t),
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
                let idx = self.gen_expr_ir(expr, &self.size_of(&func_ret_ty));
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
            let ty = self.size_of(&def_args[index].ty).clone();
            let idx = self.gen_expr_ir(expr_arg, &ty);
            func_meta_data.insert_param_parent_id(idx);
        }
        inst::Inst::CallFunc(func_meta_data)
    }


    fn gen_expr_ir(&mut self, expr: node::Expr, expect_byte: &types::Size) -> usize {
        let inst = match expr {
            // ポインタ関係
            node::Expr::GetAddress(target) => {
                let idx = self.gen_expr_ir(*target, &expect_byte);
                inst::Inst::GetAddress(idx)
            }
            node::Expr::ConnectAddr(target) => {
                let idx = self.gen_expr_ir(*target, &expect_byte);
                inst::Inst::Pointer(idx)
            }

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
            node::Expr::Assign(assign_node) => {
                println!("{:?}", assign_node);
                let right_expr_idx = self.gen_expr_ir(*assign_node.value, &expect_byte);
                let dst_idx = self.gen_expr_ir(*assign_node.dst, &expect_byte);

                inst::Inst::AssignVar {
                    name: assign_node.name.to_string(),
                    dst: dst_idx,
                    value: right_expr_idx,
                }
            }
            node::Expr::Str(value) => {
                inst::Inst::Str{
                    dst: self.id_counter,
                    value,
                }
            } 
            // ポインタの中身
            node::Expr::DefVar(mut var) => {
                match &var.ty.clone() {
                    node::TyNode::Stack{..} | node::TyNode::Static{..} => {
                        self.gen_mem_def_var(var)
                    }
                    node::TyNode::Ty(ref ty_name) => {
                        let value_idx = self.gen_expr_ir(
                            *var.value,
                            &self.size_of(&var.ty)
                        );
                        self.var_tree.push::<'l'>(&var.name, &value_idx, &var.ty);
                        inst::Inst::Mov{
                            name: Some(mem::take(&mut var.name)),
                            size: self.size_of(&node::TyNode::Ty(ty_name.to_string())),
                            dst: self.id_counter,
                            src: value_idx,
                        }
                    }
                    node::TyNode::Pointer { is_const, ty_name } => {
                        let value_idx = self.gen_expr_ir(
                            *var.value,
                            &self.size_of(&var.ty)
                        );
                        self.var_tree.push::<'l'>(&var.name, &value_idx, &var.ty);
                        inst::Inst::Mov{
                            name: Some(mem::take(&mut var.name)),
                            size: types::Size::build_ptr_ty(&*ty_name),
                            dst: self.id_counter,
                            src: value_idx,
                        }
                    }
                    t => panic!("{:?}", t),
                }
            }
            node::Expr::CallFunc(meta_data) => {
                self.gen_call_func_ir(None, &meta_data)
            }
            node::Expr::Var(name) => {
                return match self.var_tree.get(&name) {
                    def_tree::VarType::Local(index) => {
                        *index
                    }
                    def_tree::VarType::Param(param) => {
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
            // 列挙型のメンバへのアクセス: `Name::Mem`
            // メンバの定義順に基いたタグ(整数値)として展開する
            node::Expr::EnumVariant { name, variant } => {
                let enum_def = self.enum_tree
                    .get(&name)
                    .unwrap_or_else(|| panic!("未定義の列挙型です: {}", name));
                let variant_index = enum_def
                    .variants
                    .iter()
                    .position(|v| v == &variant)
                    .unwrap_or_else(|| panic!(
                        "列挙型 `{}` にメンバ `{}` は存在しません", name, variant
                    ));
                inst::Inst::gen_num(&variant_index.to_string(), &expect_byte, self.id_counter)
            }
            // 構造体の初期化: `Name { field: value, .. }`
            node::Expr::InitStruct { name, mut fields } => {
                let struct_def = self.struct_tree
                    .get(&name)
                    .clone()
                    .unwrap_or_else(|| panic!("未定義の構造体です: {}", name));

                let mut mem_insts = Vec::with_capacity(struct_def.fields.len());
                // フィールドは構造体で定義された順番通りに展開する
                for field in struct_def.fields.clone().iter() {
                    let field_size = self.size_of(&field.ty);
                    let field_expr = fields
                        .remove(&field.name)
                        .unwrap_or_else(|| panic!(
                            "構造体 `{}` の初期化にフィールド `{}` の値がありません", name, field.name
                        ));
                    let value_idx = self.gen_expr_ir(*field_expr, &field_size);
                    mem_insts.push(inst::MemoryInst::Member {
                        parent: field.name.clone(),
                        value_idx: value_idx,
                        size: field_size,
                    });
                }
                inst::Inst::Struct(mem_insts)
            }
            node::Expr::Scope { scope, target } => {
                if let node::Expr::CallFunc(call_func_node) = *target {
                    self.gen_call_func_ir(scope.last(), &call_func_node)
                } else {
                    panic!();
                }
            }
            node::Expr::Member { scope, target } => {
                match &*target {
                    node::Expr::Var(name) => {
                        let _ = match self.var_tree.get(&scope.last().unwrap()) {
                            def_tree::VarType::Local(index) => {
                                *index
                            }
                            def_tree::VarType::Param(param) => {
                                // 引数のノード
                                *param
                            }
                        };
                        let size = self.struct_tree.get_pos(
                            &scope.last().unwrap(),
                            &name,
                        );
                        inst::Inst::RefStruct{
                            src: scope.last().unwrap().to_string(),
                            size
                        }
                    }
                    t => panic!("{:?}", t),
                }
            }
            _ => panic!(),
        };

        self.ir_tree.push(inst);

        self.id_counter += 1;
        self.id_counter - 1
    }

    /// スタック/静的領域に確保する変数の定義から
    /// `inst::Inst::Struct(Vec<inst::MemoryInst>)` を生成する
    ///
    /// - `name: [ty] = 100` のように長さの指定が無い場合は要素数1
    /// - `name: [ty 4] = {100, 100, 100, 100}` のように配列リテラルが
    ///   与えられた場合は、要素ごとに値を生成する
    fn gen_mem_def_var(&mut self, mut var: node::DefineVar) -> inst::Inst {
        let (ref ty_name, len, is_static) = match &var.ty {
            node::TyNode::Stack { name, len } => (name.clone(), *len, false),
            node::TyNode::Static { name, len } => (name.clone(), *len, true),
            _ => panic!("gen_mem_def_var: スタック/静的領域の型ではありません"),
        };
        let elem_size = self.size_of(&node::TyNode::Ty(ty_name.to_string()));

        // 配列リテラルなら要素ごとに展開し、単一の値ならそのまま1要素として扱う
        let values = match *var.value {
            node::Expr::Array(items) => items,
            other => vec![other],
        };

        let mut last_value_idx = None;
        for value in values {
            last_value_idx = Some(self.gen_expr_ir(value, &elem_size));
        }

        // メモリ領域の情報を作成
        // 先頭にどの領域(スタック/静的)かを示し、続けて要素分のバイト数を積む
        let mem_kind = 
            if is_static {
                inst::MemoryKind::Static
            } else {
                inst::MemoryKind::Stack
            };
        let mem_insts = 
            inst::MemoryInst::Memory{
                name: var.name.to_string(),
                size: elem_size.clone(),
                src: last_value_idx.unwrap(),
                kind: mem_kind,
                dst: self.ir_tree.len(),
            };
        for _ in 0..len {
            //mem_insts.push(inst::MemoryInst::Byte(elem_size.to_bytes()));
        }

        // 変数はスタック/静的領域に配置された値を指す必要があるため、
        // 生成した値そのもの(last_value_idx)ではなく、
        // この`MemoryValue`命令自身のid(=mem_insts.dst)を
        // 変数の指す先として登録する。
        // (以前はlen == 1の場合に値のidをそのまま使っていたため、
        //  変数を参照した際に即値やレジスタが直接使われてしまい、
        //  スタック/静的領域への書き込みが無視されるバグがあった)
        let var_idx = self.id_counter;
        self.var_tree.push::<'l'>(&mem::take(&mut var.name), &var_idx, &var.ty);

        inst::Inst::MemoryValue(mem_insts)
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

    #[cfg(test)]
    pub(crate) fn test_only_get_func_body(&self, name: &str) -> Vec<inst::Inst> {
        self.func_tree.func.get(name).unwrap().body.clone()
    }

    /// 関数のノードを生成するときに、引数を登録
    fn push_param_meta_data(&mut self, params: &Vec<node::ArgsNode>) {
        for (index, param) in params.iter().enumerate() {
            self.var_tree.push::<'p'>(
                &param.name,
                &index,
                &param.ty
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
mod mem_var_tests {
    use super::*;
    use crate::{lex, parse};

    fn build_func_body(src: &str) -> Vec<inst::Inst> {
        let mut lexer = lex::Lexer::new();
        lexer.analy(&src.to_string()).unwrap();
        let mut parser = parse::Parser::new();
        let nodes = parser.parser(lexer.gen_tkns.clone()).unwrap().clone();
        let mut ir = IR::new();
        ir.builder(&nodes).unwrap();
        ir.test_only_get_func_body("main")
    }

    #[test]
    fn check_stack_struct_inst() {
        let body = build_func_body("main(): int { a: [int] = 100 }");
        let name = String::from("a");
        assert!(
            body.iter().any(|inst| matches!(
                inst,
                inst::Inst::MemoryValue(
                    inst::MemoryInst::Memory {
                        name: name,
                        size: types::Size::DD,
                        src: 0,
                        kind: inst::MemoryKind::Stack,
                        dst: 1,
                    }
                )
            )),
            "{:?}", body
        );
    }

    /*#[test]
    fn check_stack_array_struct_inst() {
        let body = build_func_body("main(): int { a: [int 4] = [100, 100, 100, 100] }");
        assert!(
            body.iter().any(|inst| matches!(
                inst,
                inst::Inst::Struct(v) if v == &vec![
                    inst::MemoryInst::Stack,
                    inst::MemoryInst::Byte(4),
                    inst::MemoryInst::Byte(4),
                    inst::MemoryInst::Byte(4),
                    inst::MemoryInst::Byte(4),
                ]
            )),
            "{:?}", body
        );
    }*/

    #[test]
    fn check_static_struct_inst() {
        let body = build_func_body("main(): int { a: \"\"[int] = 100 }");
        let name = String::from("a");
        assert!(
            body.iter().any(|inst| matches!(
                inst,
                inst::Inst::MemoryValue(
                    inst::MemoryInst::Memory {
                        name: name,
                        size: types::Size::DD,
                        src: 0,
                        kind: inst::MemoryKind::Static,
                        dst: 1,
                    }
                )
            )),
            "{:?}", body
        );
    }

    #[test]
    fn check_enum_as_ty_and_variant_access() {
        // 列挙型を型として使い、`Name::Mem`でメンバにアクセスできる
        let body = build_func_body(
            "enum Color { Red, Green, Blue } main(): int { a: Color = Color::Green }"
        );
        assert!(
            body.iter().any(|inst| matches!(
                inst,
                inst::Inst::Num { value, .. } if value == "1"
            )),
            "Color::Green は2番目(index 1)のメンバとして展開される必要がある: {:?}", body
        );
    }

    #[test]
    fn check_enum_forward_reference() {
        // 列挙型が使われる場所より後に定義されていても解決できる
        let body = build_func_body(
            "main(): int { a: Color = Color::Blue } enum Color { Red, Green, Blue }"
        );
        assert!(
            body.iter().any(|inst| matches!(
                inst,
                inst::Inst::Num { value, .. } if value == "2"
            )),
            "{:?}", body
        );
    }

    #[test]
    fn check_combined_struct_enum_program() {
        // 構造体と列挙型を同じプログラム内で型として使えることを確認する
        let body = build_func_body(
            "enum Color { Red, Green, Blue } struct Point { x: int, y: int } main(): int { c: Color = Color::Green p: Point = Point { x: 1, y: 2 } }"
        );
        assert!(body.iter().any(|i| matches!(i, inst::Inst::Num{value, ..} if value == "1")));
        assert!(body.iter().any(|i| matches!(i, inst::Inst::Struct(m) if m.len() == 2)));
    }

    #[test]
    fn check_struct_as_ty_and_init() {
        // 構造体を型として使い、フィールドを初期化できる
        let body = build_func_body(
            "struct Point { x: int, y: int } main(): int { p: Point = Point { x: 1, y: 2 } }"
        );
        assert!(
            body.iter().any(|inst| matches!(
                inst,
                inst::Inst::Struct(members) if members.len() == 2
                    && members.iter().any(|m| matches!(
                        m,
                        inst::MemoryInst::Member { parent, .. } if parent == "x"
                    ))
                    && members.iter().any(|m| matches!(
                        m,
                        inst::MemoryInst::Member { parent, .. } if parent == "y"
                    ))
            )),
            "{:?}", body
        );
    }
}
