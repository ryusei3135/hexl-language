//! アセンブリ言語のフォーマット関係
use std::fs;
use std::collections::HashMap;

use serde::{Serialize, Deserialize};

use crate::gen;
use crate::ir;


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
    pub extern_def: String,
    pub ret: usize,
    pub call: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FuncArgsReg {
    pub fmt: HashMap<String, Vec<usize>>
}

/// 構造体のフォーマット
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DataFmt {
    pub head: String,
    pub fmt: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OpSizeFmt {
    pub db: String,
    pub dw: String,
    pub dd: String,
    pub dq: String
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ValueFmt {
    pub reg: String,
    pub num: String,
    pub static_var: String,
    pub string: String,
    pub global: String,
    pub data: DataFmt,
    pub op_size: OpSizeFmt,
    pub ref_stack: String,
    pub frame: String,
    pub frame_end: String,
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
    pub entry: String,
}

impl AsmSetting {
    pub fn get_asm_fmt(&self, inline_name: &Option<String>) -> AsmFormat {
        // フォーマットするアセンブリコードの情報が入ったファイルの名前を取得
        let file_name = || -> &str {
            if let Some(name) = &inline_name {
                self.settings.iter().find(|v| &v.name == name).unwrap().file.as_str()
            } else {
                self.settings[self.default].file.as_str()
            }
        };

        let asm_fmt_file_name = format!("asm_fmts/{}", file_name());

        serde_json::from_str(
            &fs::read_to_string(
                &asm_fmt_file_name
            )
            .expect(&asm_fmt_file_name)
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
pub fn gen_asm_text(
    mut tree: ir::def_tree::FuncTree,
    extern_funcs: &Vec<ir::inst::Inst>,
    global_funcs: &Vec<String>,
    inline_name: &Option<String>
) -> String {
    let asm_settings = load_setting();

    let asm_fmt = asm_settings.get_asm_fmt(inline_name);
    let mut writer = gen::AsmEmitter::new(asm_settings, asm_fmt);
    writer.to_asm_text(
        &mut tree,
        &inline_name,
        &extern_funcs,
        &global_funcs
    )
}
