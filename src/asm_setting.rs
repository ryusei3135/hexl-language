//! アセンブリ言語のフォーマット関係
use std::collections::HashMap;
use std::fs;

use serde::{Deserialize, Serialize};

use crate::OptSettings;
use crate::compiler::asm_gen;
use crate::compiler::ir;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Reg {
    pub db: Vec<String>,
    pub dw: Vec<String>,
    pub dd: Vec<String>,
    pub dq: Vec<String>,
    pub xmm: Vec<String>,
    pub ymm: Vec<String>,
    pub zmm: Vec<String>,
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
    pub fmt: HashMap<String, Vec<usize>>,
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
    pub dq: String,
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
    #[serde(default = "default_mnemonic_size")]
    pub mnemonic_size: bool,
    pub ref_stack: String,
    pub frame: String,
    pub frame_end: String,
}

/// なんの関数か忘れた
fn default_mnemonic_size() -> bool {
    true
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

/// アセンブリ設定まわりで起きるエラー。
/// panic / unwrap せず、呼び出し元 (`lib.rs`) まで伝搬して表示する。
#[derive(Debug)]
pub enum AsmSettingError {
    /// 設定ファイル(`asm.yaml` / `asm_fmts/*.yaml`)を読み込めない
    ReadFile { path: String, source: std::io::Error },
    /// 設定ファイルのyamlが不正
    ParseYaml { path: String, source: serde_yaml::Error },
    /// 登録されていない inline アセンブラ名
    UnknownInline { name: String, available: Vec<String> },
    /// `default` が `settings` の範囲外
    InvalidDefault { default: usize, len: usize },
    /// アセンブリコードの生成に失敗
    Emit(String),
}

impl std::fmt::Display for AsmSettingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReadFile { path, source } => {
                write!(f, "アセンブリ設定ファイル`{}`を読み込めません: {}", path, source)
            }
            Self::ParseYaml { path, source } => {
                write!(f, "アセンブリ設定ファイル`{}`のyamlが不正です: {}", path, source)
            }
            Self::UnknownInline { name, available } => write!(
                f,
                "登録されていない inline アセンブラ名です: `{}` (登録済み: {})",
                name,
                available.join(", ")
            ),
            Self::InvalidDefault { default, len } => write!(
                f,
                "asm.yaml の default={} が settings の範囲外です (settings は {} 個)",
                default, len
            ),
            Self::Emit(msg) => write!(f, "アセンブリコードの生成に失敗しました: {}", msg),
        }
    }
}

impl std::error::Error for AsmSettingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ReadFile { source, .. } => Some(source),
            Self::ParseYaml { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl AsmSetting {
    pub fn get_asm_fmt(&self, inline_name: Option<&str>) -> Result<AsmFormat, AsmSettingError> {
        // フォーマットするアセンブリコードの情報が入ったファイルの名前を取得
        let file_name: &str = if let Some(name) = inline_name {
            self.settings
                .iter()
                .find(|v| v.name == name)
                .ok_or_else(|| AsmSettingError::UnknownInline {
                    name: name.to_string(),
                    available: self.get_inline_asm_list(),
                })?
                .file
                .as_str()
        } else {
            self.settings
                .get(self.default)
                .ok_or(AsmSettingError::InvalidDefault {
                    default: self.default,
                    len: self.settings.len(),
                })?
                .file
                .as_str()
        };

        let path = format!("asm_fmts/{}", file_name);

        let text = fs::read_to_string(&path).map_err(|source| AsmSettingError::ReadFile {
            path: path.clone(),
            source,
        })?;
        serde_yaml::from_str(&text).map_err(|source| AsmSettingError::ParseYaml { path, source })
    }

    pub fn get_inline_asm_list(&self) -> Vec<String> {
        self.settings.clone().into_iter().map(|v| v.name).collect()
    }
}

pub fn load_setting() -> Result<AsmSetting, AsmSettingError> {
    let path = "asm.yaml";
    let setting = fs::read_to_string(path).map_err(|source| AsmSettingError::ReadFile {
        path: path.to_string(),
        source,
    })?;
    serde_yaml::from_str(&setting).map_err(|source| AsmSettingError::ParseYaml {
        path: path.to_string(),
        source,
    })
}

/// アセンブリ言語のフォーマットを取得し、生成する
pub fn gen_asm_text(
    mut tree: ir::def_tree::FuncTree,
    extern_funcs: &[ir::inst::Inst],
    global_funcs: &[String],
    inline_name: Option<&str>,
    os: &str,
) -> Result<String, AsmSettingError> {
    if inline_name.is_some() {
        let asm_settings = load_setting()?;

        let asm_fmt = asm_settings.get_asm_fmt(inline_name)?;
        let mut writer = asm_gen::AsmEmitter::new(asm_settings, Some(asm_fmt), os);
        Ok(writer.to_asm_text(tree, inline_name, &extern_funcs, &global_funcs))
    } else {
        println!("現在アセンブリコードのフォーマットをハードコード `asm_setting`");
        let asm_settings = load_setting()?;
        let mut writer = asm_gen::AsmEmitter::new(asm_settings, None, os);
        Ok(writer.to_asm_text(tree, inline_name, &extern_funcs, &global_funcs))
    }
}