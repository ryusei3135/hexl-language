//! このモジュールは、IRで生成したものを
//! 渡されたフォーマットのアセンブリ言語に変換するApiを提供する

/// self.format_lineで構造体のポインタを渡すところがある
mod asm_emitter;
mod call_func;
mod emit_fn_name;
mod inline_asm;
mod asm_fmt;
mod reg_mnger;

use crate::compiler::ir::{def_tree, inst, types::Size};
use std::collections::HashMap;
use std::mem;
use crate::compiler::models::AsmFmtData;

pub type SelfPtrInfo = Option<Size>;

pub struct AsmEmitter {
    pub(super) asm_text: String,
    pub(super) data_sec_text: String,
    pub(super) data_idx: usize,
    pub(super) reg_idx: usize,
    pub(super) expr_vars: Vec<String>,
    pub(super) reserved_label_name: Option<String>,

    pub(super) asm_fmt: asm_fmt::mng_fmt::MngAsmFmt,

    pub(in crate::compiler::asm_gen) curr_inst: Vec<inst::Inst>,
    // (親のid, 変数の名前)
    pub(super) data_map: Vec<(usize, String)>,
    pub(super) last_inst_idx: Vec<(usize, usize)>,
    pub(super) var_hash_map: HashMap<String, asm_emitter::VarIndexInfo>,
    // 現在使用中のレジスタを管理する
    pub(super) used_reg: reg_mnger::UsedRegManager,
    /// `StartScope`の時点で既に存在した変数の名前のスタック
    /// `EndScope`で、スコープの外の変数が使うレジスタを
    /// 解放してしまわないようにするために使う
    pub(super) scope_outer_vars: Vec<Vec<String>>,
    pub(super) stk_use_counter: usize,
    /// `build_fn_process`が、関数呼び出しのノードとして既に
    /// アセンブリを生成済みの`Inst::CallFunc`のid。
    /// 生成済みの呼び出しの戻り値を、後続のノードが値として
    /// 参照する際(`a: int = func(10)`など)に使う
    pub(super) emitted_calls: Vec<usize>,
    /// 配列の添字が構造体のメンバー(`[arr a.a else 0]`など)のとき、
    /// 添字を載せるために確保した一時レジスタ。
    /// 配列への書き込みが終わった時点で解放する
    pub(super) arr_index_temp: Option<usize>,
    /// 構造体のコンストラクタへ渡す暗黙の`self`(`Inst::GetPtr`)のノードid
    /// -> 実際に割り当てた`%rbp`からのオフセット。
    /// IRの`stk`は配列など、エミッタ側(`stk_use_counter`)が割り当てた
    /// スタック領域と重なることがあるため、構造体の実体は
    /// `stk_use_counter`から確保し直す。関数ごとにクリアする
    pub(super) struct_stk_map: HashMap<usize, usize>,
}
