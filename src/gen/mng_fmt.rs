//! 出力するアセンブリ言語のフォーマットを管理する


use super::*;
use std::any::{Any, TypeId};

/// アセンブリ言語のフォーマットをするAPIを提供
pub struct MngAsmFmt {
    param_fmt: Vec<usize>, 
    reg_fmt: Reg,
    opcode_fmt: HashMap<String, OperandInfo>,
    pub(super) asm_setting: Option<AsmSetting>,
    default: AsmFormat,
}

impl MngAsmFmt {
    pub fn new(asm_setting: AsmSetting) -> Self {
        // === アセンブラのフォーマットの設定 ===
        let default = asm_setting
            .get_default_format();
        Self {
            param_fmt: default.args.fmt.get("linux").unwrap().clone(),
            reg_fmt: default.reg.clone(),
            opcode_fmt: default.op.clone(),
            asm_setting: Some(asm_setting),
            default: default.clone(),
        }
    }

    /// 設定された、関数の引数のレジスタを返す
    pub fn get_fmt_param<R: 'static>(&self, param_idx: &usize) -> R {
        if TypeId::of::<R>() == TypeId::of::<String>() {
            let result: Box<dyn Any> = Box::new(
                self.get_fmt_reg(&self.param_fmt[*param_idx], &Size::DD)
                    .to_string()
            );
            result.downcast::<R>().ok().map(|b| *b).unwrap()
        } else if TypeId::of::<R>() == TypeId::of::<usize>() {
            let result: Box<dyn Any> = Box::new(self.param_fmt[*param_idx]);
            result.downcast::<R>().ok().map(|b| *b).unwrap()
        } else {
            panic!("この型は無効です,");
        }
    }

    pub fn get_section_fmt(&self, section: &str) -> String {
        self.default.section.replace("{}", section)
    }

    pub fn get_opcode_tmpl(&self, key: &str) -> String {
        self.opcode_fmt.get(key).unwrap().template.clone()
    }

    pub fn get_fmt_num(&self, value: &String) -> String {
        self.default.fmt.num.replace("{}", value).to_string()
    }

    pub fn get_fmt_reg(&self, reg_num: &usize, size: &Size) -> String {
        let reg = match size {
            Size::DB => {
                &self.reg_fmt.db
            }
            Size::DW => { 
                &self.reg_fmt.dw
            }
            Size::DD => { 
                &self.reg_fmt.dd
            }
            Size::DQ => { 
                &self.reg_fmt.dq
            }
        };
        self.default.fmt.reg.replace("{}", reg[*reg_num].as_str()).to_string()
    }

    pub fn inline_asm_list(&self) -> Vec<String> {
        self.asm_setting
            .as_ref()
            .unwrap()
            .get_inline_asm_list()

    }
}
