//! 1つの関数のアセンブリを生成している間だけ意味を持つ状態

use super::asm_emitter::VarIndexInfo;
use super::*;
use std::collections::{BTreeMap, HashSet};

/// 関数ごとの状態
///
/// `AsmEmitter::fn_state`として持つ。関数の生成を始めるときに`Default`へ戻し
/// (`AsmEmitter::reset_fn_state`)、呼ばれた関数を生成している間は
/// `mem::take`で丸ごと退避・復元する(`AsmEmitter::emit_callee_fn`)。
///
/// 関数ごとの状態のフィールドは、ここへ追加すれば退避・復元の対象になる。
/// (`used_reg`だけは`Default`へ戻さず`clone`で退避するので`AsmEmitter`に残している)
#[derive(Default)]
pub(in crate::compiler::asm_gen) struct FnState {
    pub(in crate::compiler::asm_gen) reg_idx: usize,
    pub(in crate::compiler::asm_gen) expr_vars: Vec<String>,
    pub(in crate::compiler::asm_gen) reserved_label_name: Option<String>,

    pub(in crate::compiler::asm_gen) curr_inst: Vec<inst::Inst>,
    // (親のid, 変数の名前)
    pub(in crate::compiler::asm_gen) data_map: Vec<(usize, String)>,
    pub(in crate::compiler::asm_gen) last_inst_idx: Vec<(usize, RegisterMode)>,
    /// スタックへ退避した式の結果ノード -> `%rbp`からのオフセット
    pub(in crate::compiler::asm_gen) spilled_expr_results: Vec<(usize, usize)>,
    pub(in crate::compiler::asm_gen) var_hash_map: HashMap<String, VarIndexInfo>,
    /// `StartScope`の時点で既に存在した変数の名前のスタック
    /// `EndScope`で、スコープの外の変数が使うレジスタを
    /// 解放してしまわないようにするために使う
    pub(in crate::compiler::asm_gen) scope_outer_vars: Vec<Vec<String>>,
    pub(in crate::compiler::asm_gen) stk_use_counter: usize,
    /// `build_fn_process`が、関数呼び出しのノードとして既に
    /// アセンブリを生成済みの`Inst::CallFunc`のid。
    /// 生成済みの呼び出しの戻り値を、後続のノードが値として
    /// 参照する際(`a: int = func(10)`など)に使う
    pub(in crate::compiler::asm_gen) emitted_calls: Vec<usize>,
    /// 使用回数が0回の変数を定義する`Inst::Mov`のノードid。
    /// `gen_node_asm`はこのノードのアセンブリを生成しない。関数ごとに
    /// `begin_fn_state`が作り直す([`reg_mnger::UsedRegManager::unused_var_nodes`])
    pub(in crate::compiler::asm_gen) unused_var_nodes: HashSet<usize>,
    /// 配列の添字が構造体のメンバー(`[arr a.a else 0]`など)のとき、
    /// 添字を載せるために確保した一時レジスタ。
    /// 配列への書き込みが終わった時点で解放する
    pub(in crate::compiler::asm_gen) arr_index_temp: Option<usize>,
    /// 構造体のコンストラクタへ渡す暗黙の`self`(`Inst::GetPtr`)のノードid
    /// -> 実際に割り当てた`%rbp`からのオフセット。
    /// IRの`stk`は配列など、エミッタ側(`stk_use_counter`)が割り当てた
    /// スタック領域と重なることがあるため、構造体の実体は
    /// `stk_use_counter`から確保し直す
    pub(in crate::compiler::asm_gen) struct_stk_map: HashMap<usize, usize>,

    /// 退避したままの変数のレジスタ -> 退避先の`%rbp`からのオフセット
    /// (レジスタの中身は壊れている。その変数を参照するノードの生成前に復元する。
    /// 詳細は`lazy_spill.rs`)
    pub(in crate::compiler::asm_gen) spilled_regs: BTreeMap<RegisterMode, usize>,
    /// レジスタ -> 退避先のオフセット(関数ごとに、レジスタ1つにつき1回だけ確保する)
    pub(in crate::compiler::asm_gen) spill_slots: BTreeMap<RegisterMode, usize>,
    /// 暗黙の`self`ポインタ(第1引数のレジスタ`%rdi`など)の退避先の、
    /// `%rbp`からのオフセット。関数の先頭で退避し、関数の呼び出しで`%rdi`が
    /// 上書きされても、以降は退避先から読み込んで使う
    pub(in crate::compiler::asm_gen) self_ptr_slot: Option<usize>,
    /// 今生成しているノードが参照する変数の名前
    pub(in crate::compiler::asm_gen) node_vars: HashSet<String>,
    /// 今のノードが参照する変数を調べられない(関数呼び出しで退避したままにしない)
    pub(in crate::compiler::asm_gen) node_vars_all: bool,
    /// スコープが終わった変数の名前(`var_hash_map`には残っている)
    pub(in crate::compiler::asm_gen) dead_vars: HashSet<String>,
}
