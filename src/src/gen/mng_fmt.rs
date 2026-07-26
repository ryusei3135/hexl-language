//! 出力するアセンブリ言語のフォーマットを管理する
//! アセンブリ言語のフォーマットを提供する:


use super::*;
use crate::asm_setting;
use std::any::{Any, TypeId};
use crate::ir::types;

/// アセンブリ言語のフォーマットをするAPIを提供
pub struct MngAsmFmt {
    param_fmt: Vec<usize>, 
    reg_fmt: asm_setting::Reg,
    opcode_fmt: HashMap<String, asm_setting::OperandInfo>,
    pub(super) asm_setting: Option<asm_setting::AsmSetting>,
    fmt: asm_setting::AsmFormat,
}

impl MngAsmFmt {
    pub fn new(
        asm_setting: asm_setting::AsmSetting,
        asm_fmt: asm_setting::AsmFormat
    ) -> Self {
        // === アセンブラのフォーマットの設定 ===
        Self {
            param_fmt: asm_fmt.args.fmt.get("linux").unwrap().clone(),
            reg_fmt: asm_fmt.reg.clone(),
            opcode_fmt: asm_fmt.op.clone(),
            asm_setting: Some(asm_setting),
            fmt: asm_fmt
        }
    }

    /// 外部に定義されている物のフォーマット
    pub fn get_extern_func(&self, name: &String) -> String {
        self.fmt.func.extern_def.replace("{name}", name)
    }

    /// 設定された、関数の引数のレジスタを返す
    /// ## Rについて
    /// - Stringの場合はレジスタが返される
    /// - usizeの場合はレジスタの番号が返される
    /// ## 引数
    /// - param_idx = 引数の場所
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

    pub fn fmt_ref_operand(&self, reg: &String, size: &usize) -> String {
        self.fmt.fmt.ref_stack
            .replace("{src}", reg.as_str())
            .replace("{size}", size.to_string().as_str())
    }

    pub fn func_frame_fmt(&self) -> String {
        self.fmt.fmt.frame.to_string()
    }

    pub fn func_frame_end(&self) -> String {
        self.fmt.fmt.frame_end.to_string()
    } 

    /// 構造体の初期化するアセンブリ言語を返す
    #[inline(always)]
    pub fn get_fmt_struct_init(&self, size: &Vec<inst::MemoryInst>) -> String {
        // 構造体のサイズを取得
        let size: usize = types::get_struct_size(&size);
        // 構造体のサイズをスタックアライメントに調整
        let alignment_size = size + (size % 8);
        self.fmt.fmt.data.head
            // 確保するスタックのサイズ 
            .replace(
                "{size}",
                &alignment_size.to_string()
            )
    }

    pub fn stack_frame_fmt(&self, size: usize) -> String {
        let alignment_size = size + (size % 8);
        self.fmt.fmt.data.head
            // 確保するスタックのサイズ 
            .replace(
                "{size}",
                &alignment_size.to_string()
            )
    }

    /// 構造体のメンバーをフォーマット
    #[inline(always)]
    pub fn get_fmt_struct_member(&self, value: String, size: &types::Size) -> String {
        let fmted = self.fmt.fmt
        .data
        .fmt
        .replace("{dst}", value.as_str())
        .replace("{size}", size.to_bytes().to_string().as_str());
        let s_fmt = match &size {
            types::Size::DB => crate::mov_size_fmt!(self, db),
            types::Size::DW => crate::mov_size_fmt!(self, dw),
            types::Size::DD => crate::mov_size_fmt!(self, dd),
            types::Size::DQ => crate::mov_size_fmt!(self, dq),
            _ => panic!()
        };
        fmted.replace("mov", &format!("mov{}", s_fmt))
    }

    pub fn mem_ref_fmt(&self, value: &String, size: &types::Size) -> String {
        let s_fmt = match &size {
            types::Size::DB => crate::mov_size_fmt!(self, db),
            types::Size::DW => crate::mov_size_fmt!(self, dw),
            types::Size::DD => crate::mov_size_fmt!(self, dd),
            types::Size::DQ => crate::mov_size_fmt!(self, dq),
            _ => panic!()
        };
        value.replace("mov", &format!("mov{}", s_fmt))
    }

    pub fn get_push(&self, reg: &String) -> String {
        self.get_opcode_tmpl(&"push".to_string()).replace("{dst}", reg)
    }

    pub fn get_pop(&self, reg: &String) -> String {
        self.get_opcode_tmpl(&"pop".to_string()).replace("{dst}", reg)
    }

    pub fn get_str_fmt(&self, value: &String, label: &String) -> String {
        self.fmt.fmt.string
            .replace("{}", value)
            .replace("{name}", &label)
    }

    pub fn get_static_num_fmt(&self, value: &String, label: &String, size: &types::Size) -> String {
        format!("{}: .long {}\n", label, value)
    }

    pub fn get_global_fmt(&self, name: &String) -> String {
        self.fmt.fmt.global
            .replace("{name}", name)
    }
    
    /// エントリーポイントを作成
    pub fn get_entry_point(&self) -> String {
        self.fmt.fmt.global
            .replace("{name}", &self.asm_setting.as_ref().unwrap().entry)
    }

    /// アセンブリ言語のセクションを定義するフォーマット
    pub fn get_section_fmt(&self, section: &str) -> String {
        self.fmt.section.replace("{name}", section)
    }

    /// オペコードのフォーマット
    pub fn get_opcode_tmpl(&self, key: &str) -> String {
        self.opcode_fmt.get(key).unwrap().template.clone()
    }

    /// 数字のフォーマット
    pub fn get_fmt_num(&self, value: &String) -> String {
        self.fmt.fmt.num.replace("{}", value).to_string()
    }

    pub fn get_call_func_fmt(&self, func_name: &String) -> String {
        self.fmt.func.call.replace("{name}", func_name)
    }

    pub fn get_fmt_reg(&self, reg_num: &usize, size: &Size) -> String {
        let reg = match &size {
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
            Size::Struct(_) => panic!(),
        };
        self.fmt.fmt.reg.replace("{}", reg[*reg_num].as_str()).to_string()
    }

    pub fn inline_asm_list(&self) -> Vec<String> {
        self.asm_setting
            .as_ref()
            .unwrap()
            .get_inline_asm_list()

    }
}
