
use super::*;


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
            expr_counter: 0,
            // 関数の情報を初期化
            func_tree: def_tree::FuncTree::new(),
            extern_func_tree: Vec::new(),
            public_func_tree: Vec::new(),
            define_meta_data: Vec::new(),
            struct_tree: def_tree::StructTree::new(),
            enum_tree: HashMap::new(),
            stk_counter: 0,
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
            self.expr_counter = 0;
            match node {
                node::Group1Node::FuncDefine(info) => {
                    // `Self`型を、実際の構造体の型/ポインタ型へ解決する
                    let info = self.resolve_self_ty(info.clone());

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
                    // まずパスの全セグメントをファイルパスとして解釈する
                    // (例: `mod::file2` -> `mod/file2.hexl`)
                    let full_path = path.gen_path();

                    if std::path::Path::new(&full_path).exists() {
                        // ファイルとして存在する場合は、そのファイルが
                        // 公開(pub)している関数を全て、モジュール名
                        // (パスの最後のセグメント)を指定した上で
                        // extern_func_treeに登録する
                        let new_setting = settings.new_file(&full_path);
                        let mut extern_fn_tree: Vec<def_tree::FuncDefMetaData>
                            = crate::build(&new_setting).unwrap();

                        // 公開されていない関数は取り込まない
                        extern_fn_tree.retain(|v| v.public);

                        // 関数にモジュールの名前を追加
                        let module_name = path.path.last().unwrap();
                        extern_fn_tree
                            .iter_mut()
                            .for_each(|v| v.add_self_module_name(module_name));

                        self.make_extern_func_inst(&extern_fn_tree);
                        self.extern_func_tree.extend(extern_fn_tree);
                    } else {
                        // 全セグメントでのパスが存在しない場合、最後の
                        // セグメントはファイル名ではなく「関数名」と
                        // みなし、その手前までをファイルパスとして
                        // 探し直す
                        // (例: `mod::file::func` -> `mod/file.hexl`の
                        //  中にある`func`という関数)
                        let func_name = path
                            .path
                            .last()
                            .expect("#includeのパスが空です")
                            .clone();
                        let parent_path = path.gen_parent_path();

                        if !std::path::Path::new(&parent_path).exists() {
                            panic!(
                                "#includeで指定されたファイルが見つかりません: {} (\
                                関数名として`{}`も試しましたが、ファイル{}も見つかりませんでした)",
                                full_path, func_name, parent_path
                            );
                        }

                        let new_setting = settings.new_file(&parent_path);
                        let extern_fn_tree: Vec<def_tree::FuncDefMetaData>
                            = crate::build(&new_setting).unwrap();

                        // 指定された名前の、公開されている関数だけを
                        // 取り出す(モジュール名は指定しないので、
                        // そのまま`func()`のように呼び出せる)
                        let mut extern_fn_tree: Vec<def_tree::FuncDefMetaData> = extern_fn_tree
                            .into_iter()
                            .filter(|v| v.public && v.name == func_name)
                            .collect();

                        if extern_fn_tree.is_empty() {
                            panic!(
                                "#includeで指定された関数が見つかりません: {}::{}",
                                parent_path, func_name
                            );
                        }

                        self.make_extern_func_inst(&extern_fn_tree);
                        self.extern_func_tree.append(&mut extern_fn_tree);
                    }
                }
                node::Group1Node::StructDefine(info) => {
                    // 構造体の情報を登録
                    self.struct_tree.add(info);
                    // 構造体の中に定義されているメゾットを、
                    // 通常の関数としてIRへ展開する
                    self.expand_struct_methods(info);
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

    /// 構造体の中に定義されているメゾット(`StructDefine::methods`)を、
    /// 通常のトップレベルの関数として展開してIRへ変換する
    ///
    /// - メゾットは、そのメゾットが属する構造体の名前をモジュール名として
    ///   持つ関数として登録される(`node::FuncDefine::self_module_name`)。
    ///   これにより`StructName::method_name(..)`という、通常の
    ///   モジュール(`#include`)経由の関数呼び出しと同じ構文
    ///   (`node::Expr::Scope`)で呼び出せるようになる
    /// - 第一引数が`Self`型のメゾット(`self: Self`)は`resolve_self_ty`に
    ///   よって、自身の構造体を指すポインタを受け取る関数として解決される
    /// - それ以外の変換処理は、トップレベルの関数定義
    ///   (`node::Group1Node::FuncDefine`)と全く同じ流れで行う
    fn expand_struct_methods(&mut self, struct_def: &node::StructDefine) {
        for method in &struct_def.methods {
            let node::Group1Node::FuncDefine(method_info) = method else {
                panic!(
                    "構造体`{}`のメゾットに、関数定義以外のノードが渡されました: {:?}",
                    struct_def.name, method
                );
            };

            // メゾットのモジュール名を、自身が属する構造体の名前にする
            let mut method_info = method_info.clone();
            method_info.self_module_name(&struct_def.name);

            // `Self`型を、実際の構造体の型/ポインタ型へ解決する
            let method_info = self.resolve_self_ty(method_info);

            // 関数の情報を登録
            self.entry_func_info(&method_info);

            self.push_param_meta_data(&method_info.params);
            self.gen_inst(&method_info.body.clone());

            // 関数の処理内容をpush
            self.push_func_ir_tree(&method_info);
            // 使うデータを初期化
            self.ir_tree = Vec::new();
            self.id_counter = 0;
        }
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
            // `Self`はIRへ変換する前に、実際の構造体の型
            // (`node::TyNode::Ty`)やポインタ型へ解決されている必要がある
            node::TyNode::SelfTy(name) => panic!(
                "内部エラー: `Self`型(構造体`{}`)がIR変換の前に解決されていません",
                name
            ),
        }
    }

    /// 関数のノードに含まれる予約語`Self`を解決する
    ///
    /// - 第一引数の型が`Self`の場合、その関数は「メゾット」として扱い、
    ///   第一引数の型を、自身の構造体を指す**ポインタ型**に変換する
    ///   (呼び出し側は構造体のポインタをこの引数に渡すことになる)
    /// - 第一引数以外の引数の型に`Self`が使われている場合はエラー(panic)
    /// - (第一引数がSelfでない場合、)戻り値の型が`Self`ならば、
    ///   ただの関数として扱い、戻り値の型は自身の構造体の名前の型
    ///   (`node::TyNode::Ty`)として解決する
    fn resolve_self_ty(&self, mut info: node::FuncDefine) -> node::FuncDefine {
        // 第一引数以外にSelfが使われていないかチェックする
        for (index, param) in info.params.iter().enumerate() {
            if index == 0 {
                continue;
            }
            if let node::TyNode::SelfTy(struct_name) = &param.ty {
                panic!(
                    "`Self`型は第一引数(`self`)以外の引数には使用できません: \
                    関数`{}`の引数`{}` (構造体`{}`)",
                    info.name, param.name, struct_name
                );
            }
        }

        // 第一引数の型がSelfの場合、構造体のポインタとして扱う
        let has_self_param = matches!(
            info.params.first().map(|p| &p.ty),
            Some(node::TyNode::SelfTy(_))
        );

        if has_self_param {
            let node::TyNode::SelfTy(struct_name) = info.params[0].ty.clone() else {
                unreachable!()
            };
            let self_ptr_ty = node::TyNode::Pointer {
                is_const: false,
                ty_name: Box::new(node::TyNode::Ty(struct_name)),
            };
            info.params[0].ty = self_ptr_ty.clone();

            // 戻り値の型がSelfなら、`self`と同じポインタ型として解決する
            // (例: `func2(self: Self): Self { ret self }`)
            if matches!(info.ret_ty, node::TyNode::SelfTy(_)) {
                info.ret_ty = self_ptr_ty;
            }
        } else if let node::TyNode::SelfTy(struct_name) = &info.ret_ty {
            // メゾットではない(第一引数がSelfではない)が、戻り値の型がSelf
            // -> 普通の関数として定義し、戻り値は自身の構造体の名前の型になる
            // (例: `new(): Self { ret Self { .. } }`)
            info.ret_ty = node::TyNode::Ty(struct_name.clone());
        }

        info
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
            self.stk_counter,
        );
        self.stk_counter = 0;
    }

    /// 現在処理中の関数の情報を登録する
    /// **これは自分自身のファイルの中の関数**
    fn entry_func_info(&mut self, info: &node::FuncDefine) {
        self.func_ret_ty = Some(info.ret_ty.clone());
        // メゾットの場合、`info.module`に自身が属する構造体の名前が
        // 入っているので、そのままモジュール名として登録する
        self.define_meta_data.push(
            def_tree::FuncDefMetaData::new(&info, info.module.as_ref())
        );
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
            self.expr_counter = 0;
            match stmt.clone() {
                node::Group2Node::Expr(expr) => {
                    let _ = self.gen_expr_ir(expr, &types::Size::DD);
                }
                node::Group2Node::Stmt(stmt) => {
                    let _ = self.gen_stmt_ir(stmt);
                }
                // inlineアセンブラ
                node::Group2Node::CompleSyntax((name, lines)) => {
                    // それぞれの行にある`${...}`由来の式を、通常の式と
                    // 同様にIRへ変換する。構造体のメンバーやポインタの
                    // 参照/アドレス取得なども、既存の式の生成処理
                    // (`gen_expr_ir`)がそのまま扱えるので、ここでは
                    // 各オペランドを順番に渡すだけでよい
                    let asm_lines = lines
                        .into_iter()
                        .map(|line| {
                            let operand_ids = line
                                .operands
                                .into_iter()
                                .map(|expr| self.gen_expr_ir(expr, &types::Size::DD))
                                .collect::<Vec<usize>>();
                            (line.asm, operand_ids)
                        })
                        .collect::<Vec<(String, Vec<usize>)>>();

                    self.ir_tree.push(
                        inst::Inst::Comple{
                            name,
                            lines: asm_lines,
                        }
                    );
                    self.id_counter += 1;
                }
                t => println!("gen inst {:?}", t),
            }
        } 
        self.id_counter
    }

    fn stack_counter(&mut self, size: &node::TyNode) {
        self.stk_counter += types::Size::new(size).to_bytes();
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
                    .find(|v| {
                        v.name.as_str() == meta_data.name.as_str()
                            && v.module() == module_name
                    });
                    if let Some(def_data) = result {
                        def_data.gen(self.stk_counter)
                    } else {
                        panic!();
                    }
                }
            };
        let def_args = defined_func_data.args.clone();
        // 関数のノードを作成
        let mut func_meta_data
            = inst::CallFuncMetaData::new(
                meta_data.name.clone(),
                if self.expr_counter == 1 {
                    IS_NOT_ASSIGN_EXPR
                } else {
                    IS_ASSIGN_EXPR
                },
                // 確保するスタックのサイズを渡す
                if self.stk_counter != 0 {
                    Some(self.stk_counter)
                } else {
                    None
                }
            );

        for (index, _) in meta_data.args.iter().enumerate() {
            let expr_arg = meta_data.args.get(index).unwrap().clone();
            let ty = self.size_of(&def_args[index].ty).clone();
            let idx = self.gen_expr_ir(expr_arg, &ty);
            func_meta_data.insert_param_parent_id(idx);
        }
        inst::Inst::CallFunc(func_meta_data)
    }


    fn gen_expr_ir(
        &mut self, 
        expr: node::Expr, 
        expect_byte: &types::Size
    ) -> usize {
        self.expr_counter += 1;
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
            node::Expr::Equal(node) => {
                self.build_expr_inst(node, &expect_byte, inst::ExprKind::Equal)
            }
            node::Expr::NotEq(node) => {
                self.build_expr_inst(node, &expect_byte, inst::ExprKind::NotEq)
            }
            node::Expr::Number(value) => {
                inst::Inst::gen_num(&value, &expect_byte, self.id_counter)
            }
            node::Expr::Assign(assign_node) => {
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
            // 配列を初期化する
            node::Expr::Array(init_nodes) => {
                let mut dsts = Vec::new();
                for node in init_nodes.iter() {
                    let dst = self.gen_expr_ir(node.clone(), &expect_byte);
                    dsts.push(dst);
                }
                inst::Inst::InitArr(dsts)
            }
            // 配列にアクセスする
            node::Expr::RefArray { name, dst, index } => {
                let dst = self.gen_expr_ir(*dst, &expect_byte);
                inst::Inst::InsertArr {
                    name: name.to_string(),
                    dst,
                    index: self.gen_expr_ir(*index, &expect_byte),
                }
            }
            // ポインタの中身
            node::Expr::DefVar(mut var) => {
                match &var.ty.clone() {
                    node::TyNode::Stack{ .. } => {
                        // 確保するスタックを増やす
                        self.stack_counter(&var.ty);
                        self.gen_mem_def_var(var)
                    }
                    node::TyNode::Static{..} => {
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
                    node::TyNode::Pointer { ty_name, .. } => {
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
                    // 確保するスタックを増やす
                    self.stack_counter(&field.ty);

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
                            // 変数の名前で登録されている変数の型を取得する
                            // （変数）の名前の文字列
                            &self.var_tree.get_ty_name(scope.last().unwrap()),
                            &name,
                        );
                        inst::Inst::RefStruct{
                            src: scope.last().unwrap().to_string(),
                            size
                        }
                    }
                    node::Expr::CallFunc(call_func_info) => {
                        // `変数名.メゾット名(引数, ..)`という、メンバーアクセス
                        // を経由したメゾット呼び出し
                        //
                        // - `scope.last()`に入っている、呼び出し元の変数の
                        //   型(構造体名)をモジュール名として使い、
                        //   `構造体名::メゾット名`を探すのと同じ方法
                        //   (`gen_call_func_ir`)で呼び出す関数を解決する
                        // - メゾットの第一引数`self`には、呼び出し元の
                        //   変数のアドレス(構造体へのポインタ)を
                        //   暗黙的に第一引数として渡す
                        let var_name = scope.last().unwrap().clone();
                        let struct_name = self.var_tree.get_ty_name(&var_name);

                        let mut call_info = call_func_info.clone();
                        call_info.args.insert(
                            0,
                            node::Expr::GetAddress(Box::new(node::Expr::Var(var_name))),
                        );

                        self.gen_call_func_ir(Some(&struct_name), &call_info)
                    }
                    // `変数名.[メンバー名 添字]`
                    node::Expr::RefArray { name, index, .. } => {
                        let var_name = scope.last().unwrap().clone();
                        let struct_name = self.var_tree.get_ty_name(&var_name);

                        // 対象メンバーの型を取得し、要素1つ分のサイズを求める
                        let field_ty = self.struct_tree
                            .get(&struct_name)
                            .unwrap_or_else(|| panic!("未定義の構造体です: {}", struct_name))
                            .fields
                            .iter()
                            .find(|field| &field.name == name)
                            .unwrap_or_else(|| panic!(
                                "構造体 `{}` にメンバー `{}` は存在しません", struct_name, name
                            ))
                            .ty
                            .clone();
                        let elem_size = self.size_of(&field_ty).to_bytes();

                        // 配列メンバー自身の、構造体先頭から見た(1要素目までの)オフセット
                        let field_pos = self.struct_tree.get_pos(&struct_name, name);

                        // 添字は数字リテラルとしてのみ許可されているので、
                        // ここでそのまま定数として解決する
                        let index_num = match &**index {
                            node::Expr::Number(value) => {
                                value.parse::<usize>().unwrap_or_else(|_| panic!(
                                    "配列のインデックスは数字である必要があります: {}", value
                                ))
                            }
                            t => panic!("配列のインデックスは数字リテラルである必要があります: {:?}", t),
                        };

                        inst::Inst::RefStruct {
                            src: var_name,
                            size: field_pos + index_num * elem_size,
                        }
                    }
                    t => panic!("{:?}", t)// 構造体の配列型メンバーの要素にアクセスする
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

        let mut value_idx = Vec::new();
        for value in values {
            value_idx.push(self.gen_expr_ir(value, &elem_size));
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
                src: value_idx,
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
        // 各armの条件式を作成する
        // - `pattern`(matchに与えられた値)がある場合は、
        //   その値とarmのパターンを比較する式(値を与えるパターン)にする
        // - 無い場合は、armのパターンをそのまま真偽値の条件として使う
        //   (真偽値/何も与えないパターン)
        let build_cond = |arm_pattern: &node::Expr| -> node::Expr {
            match pattern {
                Some(target) => node::Expr::Equal(
                    (target.clone(), Box::new(arm_pattern.clone()))
                ),
                None => arm_pattern.clone(),
            }
        };

        // 条件分岐の塊を作成
        let end_label = if arms.len() == 1 {
            // アセンブリコードを生成するときに作成するラベル
            self.jmp_labels = self.pattern_labels + 1;
            crate::push_jmp_code!(self, ExpectJmp, self.jmp_labels);
            // 条件が一つだけの場合]
            // 条件式の生成
            let cond = build_cond(&arms[0].pattern);
            self.gen_expr_ir(cond, &types::Size::DD);
            //self.push_jmp_code(self.pattern_labels);
            self.jmp_labels.clone() + 1
        } else {
            // 条件が複数の場合
            for arm in arms.clone() {
                crate::push_jmp_code!(self, ExpectJmp, self.pattern_labels);
                let cond = build_cond(&arm.pattern);
                self.gen_expr_ir(cond, &types::Size::DD);
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

    /// 構造体のメゾットとして展開された関数の処理内容を取得する
    /// (テスト用)
    #[cfg(test)]
    pub(crate) fn test_only_get_method_body(&self, module: &str, name: &str) -> Vec<inst::Inst> {
        self.func_tree
            .func
            .get(&format!("{}::{}", module, name))
            .unwrap()
            .body
            .clone()
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
mod self_ty_tests {
    use super::*;

    fn make_func(name: &str, params: Vec<node::ArgsNode>, ret_ty: node::TyNode) -> node::FuncDefine {
        node::FuncDefine {
            public: false,
            name: name.to_string(),
            params,
            ret_ty,
            body: Vec::new(),
            module: Some("Name".to_string()),
        }
    }

    #[test]
    fn first_arg_self_becomes_pointer_to_struct() {
        // `func(self: Self, param: int): int` の第一引数は
        // 構造体`Name`へのポインタとして解決される
        let ir = IR::new();
        let func = make_func(
            "func",
            vec![
                node::ArgsNode { name: "self".to_string(), ty: node::TyNode::SelfTy("Name".to_string()) },
                node::ArgsNode { name: "param".to_string(), ty: node::TyNode::Ty("int".to_string()) },
            ],
            node::TyNode::Ty("int".to_string()),
        );

        let resolved = ir.resolve_self_ty(func);

        assert_eq!(
            resolved.params[0].ty,
            node::TyNode::Pointer {
                is_const: false,
                ty_name: Box::new(node::TyNode::Ty("Name".to_string())),
            }
        );
        // 第一引数以外はそのまま
        assert_eq!(resolved.params[1].ty, node::TyNode::Ty("int".to_string()));
    }

    #[test]
    #[should_panic]
    fn self_in_non_first_arg_panics() {
        // 第一引数以外に`Self`が使われている場合はエラー(panic)にする
        let ir = IR::new();
        let func = make_func(
            "invalid",
            vec![
                node::ArgsNode { name: "param".to_string(), ty: node::TyNode::Ty("int".to_string()) },
                node::ArgsNode { name: "self".to_string(), ty: node::TyNode::SelfTy("Name".to_string()) },
            ],
            node::TyNode::Ty("int".to_string()),
        );

        let _ = ir.resolve_self_ty(func);
    }

    #[test]
    fn no_self_arg_but_self_return_resolves_to_struct_ty() {
        // `new(): Self` のように第一引数にSelfが無く、戻り値がSelfの場合、
        // 普通の関数として扱い、戻り値は自身の構造体の名前の型になる
        let ir = IR::new();
        let func = make_func(
            "new",
            Vec::new(),
            node::TyNode::SelfTy("Name".to_string()),
        );

        let resolved = ir.resolve_self_ty(func);

        assert_eq!(resolved.ret_ty, node::TyNode::Ty("Name".to_string()));
    }

    #[test]
    fn self_arg_and_self_return_resolves_to_matching_pointer() {
        // `func2(self: Self): Self` のように、第一引数がSelfかつ
        // 戻り値もSelfの場合、戻り値も`self`と同じポインタ型になる
        let ir = IR::new();
        let func = make_func(
            "func2",
            vec![
                node::ArgsNode { name: "self".to_string(), ty: node::TyNode::SelfTy("Name".to_string()) },
            ],
            node::TyNode::SelfTy("Name".to_string()),
        );

        let resolved = ir.resolve_self_ty(func);

        let expected_ptr = node::TyNode::Pointer {
            is_const: false,
            ty_name: Box::new(node::TyNode::Ty("Name".to_string())),
        };
        assert_eq!(resolved.params[0].ty, expected_ptr.clone());
        assert_eq!(resolved.ret_ty, expected_ptr);
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
            "enum Color { Red Green Blue } main(): int { a: Color = Color::Green }"
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
            "main(): int { a: Color = Color::Blue } enum Color { Red Green Blue }"
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
            "enum Color { Red Green Blue } struct Point { x: int y: int } main(): int { c: Color = Color::Green p: Point = Point { x: 1 y: 2 } }"
        );
        assert!(body.iter().any(|i| matches!(i, inst::Inst::Num{value, ..} if value == "1")));
        assert!(body.iter().any(|i| matches!(i, inst::Inst::Struct(m) if m.len() == 2)));
    }

    #[test]
    fn check_struct_as_ty_and_init() {
        // 構造体を型として使い、フィールドを初期化できる
        let body = build_func_body(
            "struct Point { x: int y: int } main(): int { p: Point = Point { x: 1 y: 2 } }"
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

#[cfg(test)]
mod match_expr_ir_tests {
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
    fn check_match_bool_generates_equal_cmp() {
        // 1. 真偽値(比較式)を与えるパターンは、その式がそのまま
        //    条件分岐の判定に使われる (単純なif/else)
        let body = build_func_body(
            "main(): int { 
                a: int = 10
                cond a == 10 {
                    b: int = 1
                } | {
                    b: int = 0 
                } 
            }"
        );
        assert!(
            body.iter().any(|inst| matches!(
                inst,
                inst::Inst::Expr(inst::ExprInst { kind: inst::ExprKind::Equal, .. })
            )),
            "`a == 10` の比較命令(Equal)が生成される必要がある: {:?}", body
        );
    }

    #[test]
    fn check_match_value_generates_equal_cmp() {
        // 2. 値を与えるパターンでは、各armの値とmatch対象の値を
        //    Equalで比較した条件式が生成される
        let body = build_func_body(
            "main(): int {
                a: int = 10
                cond a {
                    10 => {
                        b: int = 1 
                    } 
                    20 => { 
                        b: int = 2 
                    } 
                    | => {
                        b: int = 0
                    } 
                } 
            }"
        );
        assert!(
            body.iter().filter(|inst| matches!(
                inst,
                inst::Inst::Expr(inst::ExprInst { kind: inst::ExprKind::Equal, .. })
            )).count() >= 2,
            "各armごとに`a`との比較命令(Equal)が生成される必要がある: {:?}", body
        );
    }
}

#[cfg(test)]
mod struct_method_expand_tests {
    use super::*;

    /// `self`を第一引数に取るメゾットを持つ構造体`StructDefine`を作る
    fn make_struct_with_method(
        struct_name: &str,
        method_name: &str,
        method_params: Vec<node::ArgsNode>,
        method_ret_ty: node::TyNode,
        method_body: Vec<node::Group2Node>,
    ) -> node::StructDefine {
        let method = node::FuncDefine {
            public: true,
            name: method_name.to_string(),
            params: method_params,
            ret_ty: method_ret_ty,
            body: method_body,
            // 展開前は、どのモジュールにも属していない
            module: None,
        };

        node::StructDefine {
            name: struct_name.to_string(),
            fields: vec![node::StructField::make_field("x", "int")],
            methods: vec![node::Group1Node::FuncDefine(method)],
        }
    }

    #[test]
    fn method_is_registered_as_function_with_struct_as_module() {
        // 構造体`Point`のメゾット`get_num`が、モジュール名`Point`を
        // 持つ通常の関数としてIRに展開されることを確認する
        let struct_def = make_struct_with_method(
            "Point",
            "get_num",
            vec![
                node::ArgsNode {
                    name: "self".to_string(),
                    ty: node::TyNode::SelfTy("Point".to_string()),
                },
            ],
            node::TyNode::Ty("int".to_string()),
            vec![node::StmtNode::Return(node::Expr::Number("1".to_string())).wrap()],
        );

        let mut ir = IR::new();
        ir.builder(&vec![node::Group1Node::StructDefine(struct_def)]).unwrap();

        // `Point::get_num`として展開され、呼び出せる状態になっている
        let body = ir.test_only_get_method_body("Point", "get_num");
        assert!(
            body.iter().any(|inst| matches!(inst, inst::Inst::Ret(_))),
            "メゾットの中身がそのままIRに変換されている必要がある: {:?}", body
        );

        // メゾットの第一引数(`self`)は、構造体`Point`へのポインタとして
        // 解決されている必要がある
        let registered = ir.func_tree.get(
            &"get_num".to_string(),
            Some(&"Point".to_string()),
        ).unwrap();
        assert_eq!(
            registered.args[0].ty,
            node::TyNode::Pointer {
                is_const: false,
                ty_name: Box::new(node::TyNode::Ty("Point".to_string())),
            }
        );

        // モジュール名(構造体名)を指定しない場合は解決できない
        // (トップレベルの関数と名前空間が混ざらないようにするため)
        assert!(ir.func_tree.get(&"get_num".to_string(), None).is_none());
    }

    #[test]
    fn method_can_be_called_via_struct_name_scope() {
        // `self`を取らない、構造体の「静的メゾット」を定義し、
        // `Point::answer()`のように呼び出せることを確認する
        let struct_def = make_struct_with_method(
            "Point",
            "answer",
            vec![],
            node::TyNode::Ty("int".to_string()),
            vec![node::StmtNode::Return(node::Expr::Number("42".to_string())).wrap()],
        );

        let main_fn = node::FuncDefine {
            public: true,
            name: "main".to_string(),
            params: vec![],
            ret_ty: node::TyNode::Ty("int".to_string()),
            body: vec![
                node::StmtNode::Return(
                    node::Expr::Scope {
                        scope: vec!["Point".to_string()],
                        target: Box::new(node::Expr::CallFunc(node::CallInfo {
                            name: "answer".to_string(),
                            args: vec![],
                        })),
                    }
                ).wrap()
            ],
            module: None,
        };

        let nodes = vec![
            node::Group1Node::StructDefine(struct_def),
            node::Group1Node::FuncDefine(main_fn),
        ];

        let mut ir = IR::new();
        ir.builder(&nodes).unwrap();

        let body = ir.test_only_get_func_body("main");
        assert!(
            body.iter().any(|inst| matches!(inst, inst::Inst::CallFunc(_))),
            "`Point::answer()`の呼び出しが生成される必要がある: {:?}", body
        );
    }

    #[test]
    fn methods_with_same_name_on_different_structs_do_not_collide() {
        // 別々の構造体が同じ名前(`new`)のメゾットを持っていても、
        // モジュール名(構造体名)によって区別され、互いを
        // 上書きしないことを確認する
        let point_def = make_struct_with_method(
            "Point",
            "new",
            vec![],
            node::TyNode::Ty("int".to_string()),
            vec![node::StmtNode::Return(node::Expr::Number("1".to_string())).wrap()],
        );
        let rect_def = make_struct_with_method(
            "Rect",
            "new",
            vec![],
            node::TyNode::Ty("int".to_string()),
            vec![node::StmtNode::Return(node::Expr::Number("2".to_string())).wrap()],
        );

        let nodes = vec![
            node::Group1Node::StructDefine(point_def),
            node::Group1Node::StructDefine(rect_def),
        ];

        let mut ir = IR::new();
        ir.builder(&nodes).unwrap();

        let point_new = ir.test_only_get_method_body("Point", "new");
        let rect_new = ir.test_only_get_method_body("Rect", "new");

        assert!(point_new.iter().any(|i| matches!(
            i, inst::Inst::Num { value, .. } if value == "1"
        )));
        assert!(rect_new.iter().any(|i| matches!(
            i, inst::Inst::Num { value, .. } if value == "2"
        )));
    }
}

#[cfg(test)]
mod method_call_via_member_tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn calling_method_via_dot_syntax_passes_implicit_self_pointer() {
        // `p.get_num()`のように、変数のメンバーアクセス経由で
        // メゾットを呼び出すと、暗黙的に`p`のアドレスが
        // 第一引数(`self`)として渡されることを確認する
        let method = node::FuncDefine {
            public: true,
            name: "get_num".to_string(),
            params: vec![
                node::ArgsNode {
                    name: "self".to_string(),
                    ty: node::TyNode::SelfTy("Point".to_string()),
                },
            ],
            ret_ty: node::TyNode::Ty("int".to_string()),
            body: vec![node::StmtNode::Return(node::Expr::Number("7".to_string())).wrap()],
            module: None,
        };

        let struct_def = node::StructDefine {
            name: "Point".to_string(),
            fields: vec![node::StructField::make_field("x", "int")],
            methods: vec![node::Group1Node::FuncDefine(method)],
        };

        let mut fields = HashMap::new();
        fields.insert("x".to_string(), Box::new(node::Expr::Number("1".to_string())));

        let def_p = node::DefineVar::new(
            &"p".to_string(),
            node::Expr::InitStruct { name: "Point".to_string(), fields },
            &node::TyNode::Ty("Point".to_string()),
        ).wrap().wrap_group2();

        let call_method = node::StmtNode::Return(
            node::Expr::Member {
                scope: vec!["p".to_string()],
                target: Box::new(node::Expr::CallFunc(node::CallInfo {
                    name: "get_num".to_string(),
                    args: vec![],
                })),
            }
        ).wrap();

        let main_fn = node::FuncDefine {
            public: true,
            name: "main".to_string(),
            params: vec![],
            ret_ty: node::TyNode::Ty("int".to_string()),
            body: vec![def_p, call_method],
            module: None,
        };

        let nodes = vec![
            node::Group1Node::StructDefine(struct_def),
            node::Group1Node::FuncDefine(main_fn),
        ];

        let mut ir = IR::new();
        ir.builder(&nodes).unwrap();

        let body = ir.test_only_get_func_body("main");

        // `self`のアドレスを取得する`GetAddress`命令が生成されている
        assert!(
            body.iter().any(|inst| matches!(inst, inst::Inst::GetAddress(_))),
            "`p.get_num()`は暗黙的に`p`のアドレスを渡す必要がある: {:?}", body
        );
        // 関数呼び出し命令(`get_num`)が生成されている
        assert!(
            body.iter().any(|inst| matches!(
                inst,
                inst::Inst::CallFunc(inst::CallFuncMetaData{name, ..}) if name == "get_num"
            )),
            "{:?}", body
        );
    }
}
