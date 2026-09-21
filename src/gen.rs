//! このモジュールは、IRで生成したものを
//! 渡されたフォーマットのアセンブリ言語に変換するApiを提供する

/// self.format_lineで構造体のポインタを渡すところがある
mod asm_emitter;
mod call_func;
mod inline_asm;
mod mng_fmt;
mod emit_fn_name;

use crate::ir::{def_tree, inst, types::Size};
use std::collections::HashMap;
use std::mem;

pub type SelfPtrInfo = Option<Size>;


pub struct AsmEmitter {
    pub(super) asm_text: String,
    pub(super) data_sec_text: String,
    pub(super) data_idx: usize,
    pub(super) reg_idx: usize,
    pub(super) expr_vars: Vec<String>,
    pub(super) reserved_label_name: Option<String>,

    pub(super) asm_fmt: mng_fmt::MngAsmFmt,

    pub(in crate::gen) curr_inst: Vec<inst::Inst>,
    // (親のid, 変数の名前)
    pub(super) data_map: Vec<(usize, String)>,
    pub(super) last_inst_idx: Vec<(usize, usize)>,
    pub(super) var_hash_map: HashMap<String, asm_emitter::VarIndexInfo>,
    // 現在使用中のレジスタを管理する
    pub(super) used_reg: asm_emitter::UsedRegManager,
    pub(super) stk_use_counter: usize,
    /// `build_fn_process`が、関数呼び出しのノードとして既に
    /// アセンブリを生成済みの`Inst::CallFunc`のid。
    /// 生成済みの呼び出しの戻り値を、後続のノードが値として
    /// 参照する際(`a: int = func(10)`など)に使う
    pub(super) emitted_calls: Vec<usize>,
}