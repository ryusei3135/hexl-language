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


#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Reg {
   db: Vec<String>,
   dw: Vec<String>,
   dd: Vec<String>,
   dq: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OperandInfo {
    len: usize,
    template: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Func {
    ret: usize,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AsmFormat {
    reg: Reg,
    op: HashMap<String, OperandInfo>,
    func: Func,
}



#[derive(Serialize, Deserialize, Debug)]
struct AsmSetting {
    // アセンブリ言語の設定ファイル
    settings: Vec<String>,
    // デフォルトで適応するアセンブリ言語の設定
    default: usize,
}

fn load_setting() -> AsmFormat {
    let setting = fs::read_to_string("asm.json").unwrap();
    let data: AsmSetting = serde_json::from_str(&setting).unwrap();
    let x64setting = fs::read_to_string("asm_json/x64.json").unwrap();
    let x64data: AsmFormat = serde_json::from_str(&x64setting).unwrap();
    x64data
}


fn gen_asm_text(mut tree: ir::FuncTree) {
    let format = load_setting();
    let mut writer = gen::AsmEmitter::new();
    writer.to_asm_text(&mut tree, &format);
}

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let content = fs::read_to_string(args[1].clone())?;

    let mut lexer = lex::Lexer::new();
    let mut parser = parse::Parser::new();
    let mut ir_builder = ir::IR::new();

    load_setting();


    let _ = lexer.analy(&content).map_err(|v| v.print_log(&content)); 

    let nodes = parser.parser(lexer.gen_tkns.clone())
        .map_err(|v| v.print_log(&content))
        .unwrap();
    ir_builder.builder(&nodes).unwrap();
    gen_asm_text(ir_builder.func_tree);
    Ok(())
}
