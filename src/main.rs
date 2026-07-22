mod lex;
mod gen;
mod err;
mod parse;
mod node;
mod ir;
mod macros;
mod asm_setting;

use std::{fs, env, io};


// ファイルやオプション管理
pub mod cmd_line_args {
    use super::*;

    pub enum OptFlags {
        FmtAsm,
        SetFile,
    }
    
    pub struct OptSettings {
        pub fmt_name: Option<String>,
        pub file_name: Option<String>,

        opt_flags: Option<OptFlags>,
    }

    impl OptSettings {
        pub fn new(first_flag: OptFlags) -> Self {
            Self {
                fmt_name: None,
                file_name: None,

                opt_flags: Some(first_flag),
            }
        }

        pub fn new_file(&self, file_name: &String) -> Self {
            Self {
                fmt_name: self.fmt_name.clone(),
                file_name: Some(file_name.clone()),
                opt_flags: Some(OptFlags::SetFile),
            }
        }

        /// 値を渡す、フラグがすでに立っているなら成功
        pub fn set_value(
            &mut self,
            value: String
        ) -> Result<(), err::ErrKind> {
            if let Some(flag) = self.opt_flags.take() {
                let _ = match flag {
                    OptFlags::FmtAsm => {
                        self.fmt_name.insert(value)
                    }
                    OptFlags::SetFile => {
                        self.file_name.insert(value)
                    }
                };
                Ok(())
            } else {
                Err(err::ErrKind::OptErr)
            }
        }

        /// フラグを立てる
        /// もしフラグがすでに立っている場合はエラーになる
        pub fn set_flag(
            &mut self,
            flag_name: OptFlags
        ) -> Result<(), err::ErrKind> {
            if self.opt_flags.is_none() {
                let _ = self.opt_flags.insert(flag_name);
                Ok(())
            } else {
                Err(err::ErrKind::OptErr)
            }
        }
    }

    /// オプション管理
    pub fn mng_opt_cmd(args: &Vec<String>) -> OptSettings {
        let mut settings = OptSettings::new(OptFlags::SetFile);

        for (index, opt) in args.iter().enumerate() {
            // 1以下の数はオプションをつけれない
            match &index {
                0 => continue,
                1 => {
                    let _ = settings.set_value(opt.clone());
                    continue;
                }
                _ => {},
            }

            match opt.as_str() {
                "-f" => {
                    let _ = settings.set_flag(OptFlags::FmtAsm);
                }
                // フラグ以外の文字
                _ => {
                    let _ = settings.set_value(opt.clone());
                }
            }
        }
        settings
    }
}


/// もらった情報で、データを生成
/// ## 戻り値
/// 関数の戻り値は呼び出し元に現在の公開されている
/// 関数の情報の配列を返す
pub fn build(
    settings: &cmd_line_args::OptSettings,
) -> io::Result<Vec<ir::FuncDefMetaData>> {
    // 初期化
    let content = fs::read_to_string(
        &settings
            .file_name
            .as_ref()
            .unwrap()
        )
        .expect(
            format!(
                "file >> {:?}",
                settings
                    .file_name
                    .as_ref()
                    .unwrap()
            )
            .as_str()
        );


    let mut lexer = lex::Lexer::new();
    let mut parser = parse::Parser::new();
    let mut ir_builder = ir::IR::new();
    // アセンブリ言語のデータを作成
    let _ = lexer.analy(&content).map_err(|v| v.lex_err()); 

    let nodes = parser
        .parser(lexer.gen_tkns.clone())
        .unwrap();
    let func_def_meta_data = ir_builder
        .builder(
            &nodes,
            #[cfg(not(test))] &settings
        )
        .unwrap();
    let asm_text = asm_setting::gen_asm_text(
        ir_builder.func_tree,
        &ir_builder.extern_funcs,
        &ir_builder.public_func_tree,
        &settings.fmt_name
    );
    // 出力先のアセンブリ言語のファイル
    let asm_file = settings
        .file_name
        .as_ref()
        .map(|v| v.replace(".hexl", ""))
        .unwrap();
    fs::write(format!("{}.s", asm_file), asm_text).unwrap();
    Ok(func_def_meta_data)
}

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    // オプションなどの設定
    let settings = cmd_line_args::mng_opt_cmd(&args);

    asm_setting::load_setting();

    let _ = build(&settings)?;
    Ok(())
}
