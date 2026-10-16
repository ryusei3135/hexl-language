//! このモジュールは、IRで生成したものを
//! 渡されたフォーマットのアセンブリ言語に変換するApiを提供する

/// self.format_lineで構造体のポインタを渡すところがある
mod access;
use access::FRAME_BASE_REG;
mod asm_emitter;
mod asm_fmt;
mod call_func;
mod emit_fn_name;
mod inline_asm;
mod lazy_spill;
mod reg_mnger;
mod sign;

use crate::compiler::asm_gen::asm_emitter::VarIndexInfo;
use crate::compiler::ir::{def_tree, inst, types::Size};
use crate::compiler::models::AsmFmtData;
use sign::SizeSign;
use std::collections::HashMap;
use std::mem;
use std::rc::Rc;

pub type SelfPtrInfo<'a> = Option<&'a Size>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Ord, PartialOrd)]
pub(in crate::compiler::asm_gen) enum RegisterMode {
    Normal(usize),
    Float(usize),
}

impl RegisterMode {
    pub fn index(&self) -> usize {
        match self {
            Self::Normal(idx) | Self::Float(idx) => *idx,
        }
    }

    pub fn unwrap_normal(&self) -> usize {
        if let Self::Normal(idx) = self {
            *idx
        } else {
            panic!();
        }
    }
}

pub struct AsmEmitter {
    pub(super) asm_text: String,
    pub(super) data_sec_text: String,
    pub(super) data_idx: usize,
    pub(super) reg_idx: usize,
    pub(super) expr_vars: Vec<String>,
    pub(super) reserved_label_name: Option<String>,

    pub(super) asm_fmt: asm_fmt::mng_fmt::MngAsmFmt,

    pub(super) os: String,

    pub(in crate::compiler::asm_gen) curr_inst: Vec<inst::Inst>,
    // (親のid, 変数の名前)
    pub(super) data_map: Vec<(usize, String)>,
    pub(super) last_inst_idx: Vec<(usize, RegisterMode)>,
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
    /// 使用回数が0回の変数を定義する`Inst::Mov`のノードid。
    /// `gen_node_asm`はこのノードのアセンブリを生成しない。関数ごとに
    /// `begin_fn_state`が作り直す([`reg_mnger::UsedRegManager::unused_var_nodes`])
    pub(super) unused_var_nodes: std::collections::HashSet<usize>,
    /// まだ生成していない`_start`以外の関数(ラベル -> 関数の定義)。
    /// `emit_call_func`が呼び出しを生成するときに取り出して、
    /// `build_fn_process`で生成する(呼ばれない関数は残ったまま出力されない)
    pub(super) pending_funcs: HashMap<String, (String, def_tree::FnDefInfo)>,
    /// `emit_call_func`が生成した、呼ばれた側の関数のアセンブリ。
    /// 呼んだ側の関数の途中に挟まらないよう、別に溜めて最後に結合する
    pub(super) fn_asm_out: String,
    /// 出力するアセンブリ言語のフォーマットの名前
    /// (呼ばれた関数を`build_fn_process`で生成するときに使う)
    pub(super) asm_fmt_name: Option<String>,
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

    /// 退避したままの変数のレジスタ -> 退避先の`%rbp`からのオフセット
    /// (レジスタの中身は壊れている。その変数を参照するノードの生成前に復元する。
    /// 詳細は`lazy_spill.rs`)。関数ごとの状態
    pub(super) spilled_regs: std::collections::BTreeMap<RegisterMode, usize>,
    /// レジスタ -> 退避先のオフセット(関数ごとに、レジスタ1つにつき1回だけ確保する)
    pub(super) spill_slots: std::collections::BTreeMap<RegisterMode, usize>,
    /// 今生成しているノードが参照する変数の名前
    pub(super) node_vars: std::collections::HashSet<String>,
    /// 今のノードが参照する変数を調べられない(関数呼び出しで退避したままにしない)
    pub(super) node_vars_all: bool,
    /// スコープが終わった変数の名前(`var_hash_map`には残っている)
    pub(super) dead_vars: std::collections::HashSet<String>,

    callee_saved: Rc<Vec<usize>>,
    caller_saved: Rc<Vec<usize>>,
    float_callee_saved: Rc<Vec<usize>>,
    float_caller_saved: Rc<Vec<usize>>,
}
