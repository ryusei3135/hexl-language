use super::*;


impl IR {
    pub fn include_proc(
        &mut self, 
        path: &node::ModPath,
        settings: &crate::cmd_line_args::OptSettings,
    ) -> Result<(), err::ErrKind> {
        // まずパスの全セグメントをファイルパスとして解釈する
        // (例: `mod::file2` -> `mod/file2.hexl`)
        let full_path = path.gen_path();

        if std::path::Path::new(&full_path)
            .exists() 
        {
            let new_setting = settings.new_file(&full_path);
            let mut extern_fn_tree: Vec<def_tree::FuncDefMetaData> =
                crate::build(&new_setting).unwrap();

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
            let extern_fn_tree: Vec<def_tree::FuncDefMetaData> =
                crate::build(&new_setting).unwrap();

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
        Ok(())
    }
}