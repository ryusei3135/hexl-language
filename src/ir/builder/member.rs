use crate::err::undef::UndefKind::*;

use super::*;

impl IR {
    pub fn member_is_var(
        &mut self,
        scope: &[&str],
        name: &str,
    ) -> Result<inst::Inst, err::ErrKind> {
        /*match self.var_tree.get(&scope.last().unwrap()) {
            def_tree::VarType::Local(index) => *index,
            def_tree::VarType::Param(param) => {
                // 引数のノード
                *param
            }
        };*/
        let member_name = scope
            .last()
            // スコープないに」変数が存在しない
            .ok_or_else(|| crate::GenUndefErrResult!(UndefMemberInVar, name.to_string(), None))?
            .to_string();
        let pos: usize = self.struct_tree.get_pos(
            // 変数の名前で登録されている変数の型を取得する
            // （変数）の名前の文字列
            &self.var_tree.get_ty_name(&member_name)?,
            &name,
        );
        let size: types::Size = self.struct_tree.get_mem_size(
            // 変数の名前で登録されている変数の型を取得する
            // （変数）の名前の文字列
            &self.var_tree.get_ty_name(&member_name)?,
            &name,
        );
        let r = inst::Inst::RefStruct {
            src: member_name,
            size,
            pos,
        };
        Ok(r)
    }

    pub fn member_is_fn(
        &mut self,
        scope: &[&str],
        call_func_info: &node::CallInfo,
    ) -> Result<inst::Inst, err::ErrKind> {
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
        let var_name: &str = &scope
            .last()
            // スコープ内に関数が存在しない
            .ok_or_else(|| crate::GenUndefErrResult!(UndefMemberInFn, "none".to_string(), None))?;
        let struct_name = self.var_tree.get_ty_name(&var_name)?;

        let mut call_info = call_func_info.clone();
        call_info.args.insert(
            0,
            node::Expr::GetAddress(Box::new(node::Expr::Var(var_name.to_owned()))),
        );

        let r = self.gen_call_fn_ir(Some(&struct_name.to_owned()), &call_info, None)?;
        Ok(r)
    }

    /// `[ptr].member`: ポインタ`ptr`が指す構造体のメンバー(フィールド)へ
    /// アクセスする
    ///
    /// `member_is_var`と違い、`ptr`自身の場所に構造体があるのではなく、
    /// `ptr`の値(アドレス)が指す先に構造体があるので`RefStructPtr`を使う
    pub fn member_is_var_via_ptr(
        &mut self,
        ptr_name: &str,
        name: &str,
    ) -> Result<inst::Inst, err::ErrKind> {
        // `get_ty_name`はポインタ型の変数に対しては、指す先の
        // 構造体名を返す(`src/ir/def_tree.rs`)
        let struct_name = self.var_tree.get_ty_name(ptr_name)?;

        let pos: usize = self.struct_tree.get_pos(&struct_name, name);
        let size: types::Size = self.struct_tree.get_mem_size(&struct_name, name);

        let r = inst::Inst::RefStructPtr {
            src: ptr_name.to_string(),
            size,
            pos,
        };
        Ok(r)
    }

    /// `[ptr].method(..)`: ポインタ`ptr`が指す構造体のメゾットを呼び出す
    ///
    /// `member_is_fn`と違い、`ptr`はすでに構造体へのアドレスそのものを
    /// 持っているので、暗黙のself引数には`ptr`自身のアドレスではなく
    /// `ptr`の値をそのまま渡す
    pub fn member_is_fn_via_ptr(
        &mut self,
        ptr_name: &str,
        call_func_info: &node::CallInfo,
    ) -> Result<inst::Inst, err::ErrKind> {
        let struct_name = self.var_tree.get_ty_name(ptr_name)?;

        let mut call_info = call_func_info.clone();
        call_info
            .args
            .insert(0, node::Expr::Var(ptr_name.to_owned()));

        let r = self.gen_call_fn_ir(Some(&struct_name.to_owned()), &call_info, None)?;
        Ok(r)
    }

    pub fn member_is_arr_ref(
        &mut self,
        scope: &[&str],
        name: &str,
        index: &Box<node::Expr>,
    ) -> Result<inst::Inst, err::ErrKind> {
        let var_name: &str = &scope.last().unwrap();
        let struct_name = self.var_tree.get_ty_name(&var_name)?;

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
            node::Expr::Number(val) => val.parse::<usize>().unwrap_or_else(|_| {
                panic!("配列のインデックスは数字である必要があります: {}", val)
            }),
            t => panic!(
                "配列のインデックスは数字リテラルである必要があります: {:?}",
                t
            ),
        };

        Ok(inst::Inst::RefStruct {
            src: var_name.to_owned(),
            size: types::Size::new(&field_ty).unwrap(),
            pos: field_pos + index_num * elem_size,
        })
    }
}
