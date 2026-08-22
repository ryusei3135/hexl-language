use super::*;

impl IR {
    pub fn member_is_var(&mut self, scope: &Vec<String>, name: &String) -> inst::Inst {
        let _ = match self.var_tree.get(&scope.last().unwrap()) {
            def_tree::VarType::Local(index) => *index,
            def_tree::VarType::Param(param) => {
                // 引数のノード
                *param
            }
        };
        let pos = self.struct_tree.get_pos(
            // 変数の名前で登録されている変数の型を取得する
            // （変数）の名前の文字列
            &self.var_tree.get_ty_name(scope.last().unwrap()),
            &name,
        );
        let size = self.struct_tree.get_mem_size(
            // 変数の名前で登録されている変数の型を取得する
            // （変数）の名前の文字列
            &self.var_tree.get_ty_name(scope.last().unwrap()),
            &name,
        );
        inst::Inst::RefStruct {
            src: scope.last().unwrap().to_string(),
            size,
            pos,
        }
    }

    pub fn member_is_fn(
        &mut self,
        scope: &Vec<String>,
        call_func_info: &node::CallInfo,
    ) -> inst::Inst {
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

    pub fn member_is_arr_ref(
        &mut self,
        scope: &Vec<String>,
        name: &String,
        index: &Box<node::Expr>,
    ) -> inst::Inst {
        let var_name = scope.last().unwrap().clone();
        let struct_name = self.var_tree.get_ty_name(&var_name);

        // 対象メンバーの型を取得し、要素1つ分のサイズを求める
        let field_ty = self
            .struct_tree
            .get(&struct_name)
            .unwrap_or_else(|| panic!("未定義の構造体です: {}", struct_name))
            .fields
            .iter()
            .find(|field| &field.name == name)
            .unwrap_or_else(|| {
                panic!(
                    "構造体 `{}` にメンバー `{}` は存在しません",
                    struct_name, name
                )
            })
            .ty
            .clone();
        let elem_size = self.size_of(&field_ty).to_bytes();

        // 配列メンバー自身の、構造体先頭から見た(1要素目までの)オフセット
        let field_pos = self.struct_tree.get_pos(&struct_name, name);

        // 添字は数字リテラルとしてのみ許可されているので、
        // ここでそのまま定数として解決する
        let index_num = match &**index {
            node::Expr::Number(value) => value.parse::<usize>().unwrap_or_else(|_| {
                panic!("配列のインデックスは数字である必要があります: {}", value)
            }),
            t => panic!(
                "配列のインデックスは数字リテラルである必要があります: {:?}",
                t
            ),
        };

        inst::Inst::RefStruct {
            src: var_name,
            size: types::Size::new(&field_ty),
            pos: field_pos + index_num * elem_size,
        }
    }
}
