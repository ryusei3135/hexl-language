mod lex;
mod gen;
mod err;
mod parse;
mod node;
mod ir;

use std::fs;
use std::env;
use std::io;
use std::collections::HashMap;

use serde::{Serialize, Deserialize};


/// アセンブリ言語のフォーマット関係
mod asm_setting_module {
    use super::*;

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct Reg {
       pub db: Vec<String>,
       pub dw: Vec<String>,
       pub dd: Vec<String>,
       pub dq: Vec<String>,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct OperandInfo {
        pub len: usize,
        pub template: String,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct Func {
        pub ret: usize,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct FuncArgsReg {
        pub fmt: HashMap<String, Vec<usize>>
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct ValueFmt {
        pub reg: String,
        pub num: String,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct AsmFormat {
        pub reg: Reg,
        pub args: FuncArgsReg,
        pub section: String,
        pub fmt: ValueFmt,
        pub op: HashMap<String, OperandInfo>,
        pub func: Func,
    }


    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct AsmInfos {
        // アセンブリ言語の設定ファイル
        pub file: String,
        // inlineアセンブラを使うときの名前
        pub name: String,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct AsmSetting {
        // アセンブリ言語の設定ファイル
        pub settings: Vec<AsmInfos>,
        // デフォルトで適応するアセンブリ言語の設定
        pub default: usize,
    }

    impl AsmSetting {
        pub fn get_default_format(&self) -> AsmFormat {
            serde_json::from_str(
                &fs::read_to_string(
                    format!("asm_json/{}", &self.settings[self.default].file)
                )
                .unwrap()
            ).unwrap()
        }

        pub fn get_inline_asm_list(&self) -> Vec<String> {
            self.settings
                .clone()
                .into_iter()
                .map(|v| v.name)
                .collect()
        }
    }

    pub fn load_setting() -> AsmSetting {
        let setting = fs::read_to_string("asm.json")
            .expect("not found asm setting file 'asm.json'");
        serde_json::from_str(&setting).unwrap()

        //let default_format = fs::read_to_string(data.get_default_asm_file()).unwrap();
        //let x64data: AsmFormat = serde_json::from_str(&default_format).unwrap();
        //x64data
    }

    /// アセンブリ言語のフォーマットを取得し、生成する
    pub fn gen_asm_text(mut tree: ir::FuncTree) -> String {
        let asm_settings = load_setting();

        let mut writer = gen::AsmEmitter::new(asm_settings);
        writer.to_asm_text(&mut tree)
    }
}


/// ファイルやオプション管理
mod cmd_line_args {
    pub fn get_comple_file(args: &Vec<String>) -> String {
        if args.len() > 1 {
            args[1].clone()
        } else {
            panic!("please file name");
        }
    }
}


pub use asm_setting_module::*;

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();

    let mut lexer = lex::Lexer::new();
    let mut parser = parse::Parser::new();
    let mut ir_builder = ir::IR::new();

    let file_name = cmd_line_args::get_comple_file(&args);

    load_setting();


    let content = fs::read_to_string(file_name)?;


    let _ = lexer.analy(&content).map_err(|v| v.print_log(&content)); 

    let nodes = parser.parser(lexer.gen_tkns.clone())
        .map_err(|v| v.print_log(&content))
        .unwrap();
    ir_builder.builder(&nodes).unwrap();
    let asm_text = gen_asm_text(ir_builder.func_tree);

    fs::write("a.s", asm_text).unwrap();
    Ok(())
}
