use crate::node::Expr;

use super::*;


impl IR {
    pub fn include_proc(
        &mut self, 
        path: &node::ModPath,
        settings: &crate::cmd_line_args::OptSettings,
    ) -> Result<(), err::ErrKind> {
        match &path.kind {
            // 従来の書き方(`mod::file`)。ファイルとして存在するかどうかで
            // モジュールとして取り込むか、関数として取り込むかを判断する
            node::ImportKind::Auto => {
                self.include_auto(path, settings)
            }
            // `#include Name="mod/file.hexl"` / `#include "mod/file.hexl"`
            // 公開関数を全て、モジュール名(エイリアスかファイル名由来)を
            // 付けて取り込む
            node::ImportKind::Module(alias) => {
                self.include_module(path, alias.as_ref(), settings)
            }
            // `#include "mod/file.hexl"::func`
            // 指定した関数だけを、モジュール名を付けずに取り込む
            node::ImportKind::Func(func_name) => {
                self.include_func(path, func_name, settings)
            }
            // `#include "mod/file.hexl"::*`
            // 公開関数を全て、モジュール名を付けずに取り込む
            node::ImportKind::Glob => {
                self.include_glob(path, settings)
            }
        }
    }

    /// 指定されたファイルが存在するかを確認し、無ければパニックする
    #[inline(always)]
    fn check_include_file_exists(&self, full_path: &str) {
        if !std::path::Path::new(full_path).exists() {
            panic!(
                "#includeで指定されたファイルが見つかりません: {}",
                full_path
            );
        }
    }

    /// 従来の書き方(`mod::file`)用のロジック。
    /// パスの全セグメントをファイルパスとして解釈し
    /// (例: `mod::file2` -> `mod/file2.hexl`)、それが実際に
    /// 存在すればファイル全体をモジュールとして取り込み、
    /// 存在しなければ最後のセグメントを関数名とみなして
    /// その手前までのパスから、その関数だけを取り込む
    fn include_auto(
        &mut self,
        path: &node::ModPath,
        settings: &crate::cmd_line_args::OptSettings,
    ) -> Result<(), err::ErrKind> {
        let full_path = path.gen_path();

        if std::path::Path::new(&full_path)
            .exists()
        {
            let new_setting = settings
                .new_file(&full_path);
            let mut extern_fn_tree: Vec<def_tree::FnDefMetaData> =
                crate::build(&new_setting)
                    .unwrap();

            // 公開されていない関数は取り込まない
            extern_fn_tree.retain(|v| v.public);

            // 関数にモジュールの名前を追加
            let module_name = path.last_segment();
            extern_fn_tree
                .iter_mut()
                .for_each(
                    |v| {
                        v.add_self_module_name(
                            &module_name
                        )
                    }
                );

            self.make_extern_func_inst(&extern_fn_tree);
            self.extern_func_tree.extend(extern_fn_tree);
        } else {
            let func_name = path.last_segment();
            let parent_path = path
                .gen_parent_path();

            if !std::path::Path::new(&parent_path).exists() {
                panic!(
                    "#includeで指定されたファイルが見つかりません: {} (\
                    関数名として`{}`も試しましたが、ファイル{}も見つかりませんでした)",
                    full_path, func_name, parent_path
                );
            }

            let new_setting = settings.new_file(
                &parent_path
            );
            let extern_fn_tree: Vec<def_tree::FnDefMetaData> =
                crate::build(&new_setting).unwrap();

            // 指定された名前の、公開されている関数だけを
            // 取り出す(モジュール名は指定しないので、
            // そのまま`func()`のように呼び出せる)
            let mut extern_fn_tree: Vec<def_tree::FnDefMetaData> 
                = extern_fn_tree
                .into_iter()
                .filter(|v| v.public && v.name == func_name)
                .collect();

            if extern_fn_tree.is_empty() {
                panic!(
                    "#includeで指定された関数が見つかりません: {}::{}",
                    parent_path, func_name
                );
            }

            self.make_extern_func_inst(
                &extern_fn_tree
            );
            self.extern_func_tree
                .append(&mut extern_fn_tree);
        }
        Ok(())
    }

    /// `#include Name="mod/file.hexl"` / `#include "mod/file.hexl"`用の
    /// ロジック。ファイル内の公開関数を全て、モジュール名
    /// (エイリアスが指定されていればそれを、無ければファイル名から
    /// 生成した名前)を付けて取り込む
    fn include_module(
        &mut self,
        path: &node::ModPath,
        alias: Option<&String>,
        settings: &crate::cmd_line_args::OptSettings,
    ) -> Result<(), err::ErrKind> {
        let full_path = path.gen_path();
        self.check_include_file_exists(&full_path);

        let new_setting = settings.new_file(&full_path);
        let mut extern_fn_tree: Vec<def_tree::FnDefMetaData> =
            crate::build(&new_setting).unwrap();

        // 公開されていない関数は取り込まない
        extern_fn_tree.retain(|v| v.public);

        // エイリアスが指定されていればそれを、
        // 無ければファイル名から生成したモジュール名を使う
        let module_name = alias
            .cloned()
            .unwrap_or_else(|| path.last_segment());

        extern_fn_tree
            .iter_mut()
            .for_each(|v| v.add_self_module_name(&module_name));

        self.make_extern_func_inst(&extern_fn_tree);
        self.extern_func_tree.extend(extern_fn_tree);
        Ok(())
    }

    /// `#include "mod/file.hexl"::func`用のロジック。
    /// 指定された名前の、公開されている関数だけを取り出す
    /// (モジュール名は付けないので、そのまま`func()`のように呼び出せる)
    fn include_func(
        &mut self,
        path: &node::ModPath,
        func_name: &String,
        settings: &crate::cmd_line_args::OptSettings,
    ) -> Result<(), err::ErrKind> {
        let full_path = path.gen_path();
        self.check_include_file_exists(&full_path);

        let new_setting = settings.new_file(&full_path);
        let extern_fn_tree: Vec<def_tree::FnDefMetaData> =
            crate::build(&new_setting).unwrap();

        let mut extern_fn_tree: Vec<def_tree::FnDefMetaData> = extern_fn_tree
            .into_iter()
            .filter(|v| v.public && &v.name == func_name)
            .collect();

        if extern_fn_tree.is_empty() {
            panic!(
                "#includeで指定された関数が見つかりません: {}::{}",
                full_path, func_name
            );
        }

        self.make_extern_func_inst(&extern_fn_tree);
        self.extern_func_tree.append(&mut extern_fn_tree);
        Ok(())
    }

    /// `#include "mod/file.hexl"::*`用のロジック。
    /// 公開されている関数を全て、モジュール名を付けずに取り込む
    /// (そのまま`func()`のように呼び出せる)
    fn include_glob(
        &mut self,
        path: &node::ModPath,
        settings: &crate::cmd_line_args::OptSettings,
    ) -> Result<(), err::ErrKind> {
        let full_path = path.gen_path();
        self.check_include_file_exists(&full_path);

        let new_setting = settings
            .new_file(
                &full_path
            );
        let mut extern_fn_tree: Vec<def_tree::FnDefMetaData> =
            crate::build(&new_setting).unwrap();

        // 公開されていない関数は取り込まない
        extern_fn_tree.retain(|v| v.public);

        self.make_extern_func_inst(&extern_fn_tree);
        self.extern_func_tree.extend(extern_fn_tree);
        Ok(())
    }

    #[inline(always)]
    pub fn inline_proc(
        &mut self,
        lines: &Vec<node::InlineAsm>,
        name: &String,
    ) {
        let mut gen_ir = |expr: Expr| {
            let ty: types::Size = 
            // 変数のノードを取得
            if let node::Expr::Var(
                ref var_name
                ) = expr 
            {
                let ty_node = self
                    .var_tree
                    .get_ty_node(&var_name)
                    .unwrap();
                types::Size::new(&ty_node)
                    .unwrap()
            } else {
                types::Size::DD
            };
            self.gen_expr_ir(
                expr, 
                &ty
            )
        };
        let asm_lines = lines
            .into_iter()
            .map(|line| {
                let operand_ids = line
                    .operands
                    .clone()
                    .into_iter()
                    .map(&mut gen_ir)
                    .collect::<Vec<usize>>();
                (line.asm.clone(), operand_ids)
            })
            .collect::<Vec<(String, Vec<usize>)>>();

        self.ir_tree.push(inst::Inst::Comple {
            name: name.to_string(),
            lines: asm_lines,
        });
        self.id_counter += 1;
    }
}