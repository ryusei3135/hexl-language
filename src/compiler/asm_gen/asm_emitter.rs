mod arr;
mod cast;
mod data_mem_factory;
mod div_ir;
mod emit_tmpl_key;
mod expr_inst;
mod format_line;
mod gen_operand;
mod insert_fmt_reg;
mod node_ty;
///! 関数の中身を生成する関数は`src/gen/call_func.rs`にある
mod operand_txt;
mod opt;
mod scope;
mod set_cc;
mod ternary;
mod var_mnger;

use super::emit_fn_name::emit_fn_name_id;
use super::*;
use crate::CpuKind;
use crate::asm_setting;
use crate::compiler::err::{self, err_pool};
use crate::compiler::ir::inst::Inst;
use crate::compiler::ir::types;

pub use var_mnger::VarIndexInfo;

impl AsmEmitter {
    pub fn new(
        asm_setting: asm_setting::AsmSetting,
        asm_fmt: AsmFmtData,
        os: &str,
        err_pool: &Rc<RefCell<err_pool::ErrPool>>,
        opt_mode: OptMode,
        cpu: CpuKind,
    ) -> Self {
        let caller = Rc::new(vec![1, 4, 5, 6, 7, 8, 9]);
        let callee = Rc::new(vec![3, 10, 11, 12, 13]);
        let float_caller = Rc::new((0..16).collect());
        let float_callee = Rc::new(Vec::new());

        let mut me = Self {
            asm_text: String::new(),
            data_sec_text: String::new(),
            data_idx: 0,

            asm_fmt: asm_fmt::mng_fmt::MngAsmFmt::new(asm_setting, asm_fmt, os, cpu),
            os: os.to_string(),

            fn_state: FnState::default(),
            used_reg: reg_mnger::UsedRegManager::new(
                Rc::clone(&callee),
                Rc::clone(&caller),
                Rc::clone(&float_callee),
                Rc::clone(&float_caller),
            ),
            pending_funcs: HashMap::new(),
            fn_asm_out: String::new(),
            asm_fmt_name: None,

            callee_saved: Rc::clone(&callee),
            caller_saved: Rc::clone(&caller),

            float_callee_saved: Rc::clone(&float_callee),
            float_caller_saved: Rc::clone(&float_caller),

            err_pool: Rc::clone(&err_pool),

            optimization_mode: opt_mode,
        };
        me.data_sec_text
            .push_str(&me.asm_fmt.get_section_fmt("data"));
        me
    }

    pub fn to_asm_text(
        &mut self,
        mut func_tree: def_tree::FuncTree,
        asm_fmt_name: Option<&str>,
        extern_funcs: &[inst::Inst],
        global_funcs: &[String],
    ) -> String {
        // `_start`が定義されているファイルの場合だけ、エントリー
        // ポイントとして`.global _start`を出力する
        // (#includeされるライブラリ用ファイルなど、`_start`を
        //  定義しないファイルにまでこれを出力してしまうと、
        //  存在しないシンボルを公開する不正なアセンブリになる)
        self.asm_text = if func_tree.func.contains_key("_start") {
            String::from(&self.asm_fmt.get_entry_point())
        } else {
            String::new()
        };

        let mut start_func = func_tree.func.remove_entry("_start");

        self.gen_global_func_asm(&global_funcs);
        self.gen_extern_func_asm(&extern_funcs);

        // `_start`以外の関数は、`call_func.rs`の`emit_call_func`が
        // 呼び出しを生成するときに`build_fn_process`で生成する
        // (呼ばれない関数は生成しない)。ここでは`pending_funcs`へ
        // 登録するだけで、生成するのは次の関数のみ。
        //  - `_start`(エントリーポイント)
        //  - `global_funcs`(他のファイルから呼ばれるので、起点として生成する)
        self.pending_funcs.clear();
        self.fn_asm_out.clear();
        self.asm_fmt_name = asm_fmt_name.map(str::to_string);
        for (name, info) in func_tree.func.drain() {
            if name == "_start" {
                continue;
            }
            // 本体が無い関数(`#include`した型のメゾットなど)は生成しない。
            // 本体は、その型を定義したファイルのアセンブリにある
            if info.is_declaration {
                continue;
            }
            let label = Self::fn_label_of(&info.name, &info.temp_ty);
            self.pending_funcs.insert(label, (name, info));
        }

        // エントリーポイントを先頭に配置
        if let Some(ref mut meta_data) = start_func {
            self.build_fn_process(meta_data, asm_fmt_name);
        }

        // 公開する関数(未生成のもの)
        for fn_name in global_funcs {
            let label = emit_fn_name_id(fn_name);
            if let Some(mut meta_data) = self.pending_funcs.remove(&label) {
                self.emit_callee_fn(&mut meta_data);
            }
        }
        // 呼ばれた側の関数のアセンブリを、`_start`の後ろへ続ける
        let callee_asm = mem::take(&mut self.fn_asm_out);
        self.asm_text.push_str(&callee_asm);
        format!(
            "{}\n{}\n{}",
            self.data_sec_text,
            self.asm_fmt.get_section_fmt("text"),
            // 出力するすべての命令のニーモニックに、サイズ接尾辞を必ず付ける
            self.asm_fmt
                .ensure_mnemonic_size(&self.asm_text.replace("{space}", "  "))
        )
    }

    #[inline(always)]
    fn gen_global_func_asm(&mut self, global_funcs: &[String]) {
        // 自身が公開する関数を生成
        global_funcs.iter().for_each(|fn_name| {
            let label = emit_fn_name_id(fn_name);
            self.asm_text
                .push_str(self.asm_fmt.get_global_fmt(&label).as_str());
        });
    }

    #[inline(always)]
    fn gen_extern_func_asm(&mut self, extern_funcs: &[inst::Inst]) {
        extern_funcs.iter().for_each(|func| {
            if let inst::Inst::ExternFunc(name) = func {
                let label = emit_fn_name_id(name);
                self.asm_text
                    .push_str(self.asm_fmt.get_extern_func(&label).as_str());
            } else {
                println!("warning");
            }
        });
    }

    #[inline(always)]
    pub(super) fn insert_var_info(&mut self, name: &str, var: VarIndexInfo) {
        // メモリに実体がある変数(`in_mem`)と、`%rbp`相対で置かれた
        // 変数(`is_stack`)はレジスタを持たない(`offset`で保存する)ので、
        // レジスタを使用中にしない
        if !var.in_mem && !var.is_stack {
            self.used_reg.mark_used(&var.reg);
        }
        self.fn_state.expr_vars.push(name.to_string());
        self.fn_state.dead_vars.remove(name);
        self.fn_state.var_hash_map.insert(name.to_owned(), var);
    }

    /// 構造体の実体(`bytes`バイト)を置くスタック領域を`stk_use_counter`から
    /// 確保し、`GetPtr`のノード`ptr_idx`に対応付ける。
    ///
    /// ポインタ(`lea -N(%rbp), %rdi`)は先頭のメンバーを指し、メンバーは
    /// そこからアドレスが小さくなる向きに`(%rdi)`、`-4(%rdi)`のように並ぶ。
    /// 先頭のメンバー自身はポインタより上(アドレスが大きい側)にはみ出すため、
    /// 最大のメンバーのサイズ(8byte)だけ余分に確保する。
    /// `N`は現在のスタック使用量を8byteに切り上げた値に8を足した値とし、
    /// 使用量を`N + bytes`まで進める。これで先にある配列などと重ならない
    pub(in crate::compiler::asm_gen) fn alloc_struct_stk(
        &mut self,
        ptr_idx: usize,
        bytes: usize,
    ) -> usize {
        if let Some(offset) = self.fn_state.struct_stk_map.get(&ptr_idx) {
            return *offset;
        }
        let offset = self.fn_state.stk_use_counter.div_ceil(8) * 8 + 8;
        self.fn_state.stk_use_counter = offset + bytes;
        self.fn_state.struct_stk_map.insert(ptr_idx, offset);
        offset
    }

    /// `GetPtr`のノードの`%rbp`からのオフセット。`alloc_struct_stk`で
    /// 確保済みならそれを、そうでなければIRの`stk`を返す
    pub(in crate::compiler::asm_gen) fn ptr_stk(&self, ptr_idx: usize, ir_stk: usize) -> usize {
        self.fn_state
            .struct_stk_map
            .get(&ptr_idx)
            .copied()
            .unwrap_or(ir_stk)
    }

    /// `node_idx`が式(`Inst::Expr`)の場合、その結果を持っていた一時的な
    /// レジスタを解放する。親の式や代入が、その値を使い終わった時点で
    /// 呼ぶ。
    ///
    /// 変数がそのレジスタを持っている場合は解放しない。
    pub(super) fn release_expr_temp(&mut self, node_idx: usize) {
        if !matches!(
            self.fn_state.curr_inst[node_idx],
            inst::Inst::Expr(..) | inst::Inst::Cast { .. }
        ) {
            return;
        }
        if self
            .fn_state
            .spilled_expr_results
            .iter()
            .any(|(id, _)| *id == node_idx)
        {
            self.fn_state
                .spilled_expr_results
                .retain(|(id, _)| *id != node_idx);
            return;
        }
        let Some(reg) = self
            .fn_state
            .last_inst_idx
            .iter()
            .find(|(id, _)| *id == node_idx)
            .map(|(_, reg)| *reg)
        else {
            return;
        };
        if !self.is_reg_held_by_var(reg) {
            self.used_reg.release(&reg);
        }

        self.fn_state
            .last_inst_idx
            .retain(|(id, _)| *id != node_idx);
    }

    /// 現在`reg`を使っている値を、`push`で退避するためのレジスタ名の一覧
    /// (`dst`は除く)。`/`や`%`が書き換える`%rax`/`%rdx`のうち、
    /// 使用中のものを退避するために使う。
    pub(super) fn live_reserved_regs(&self, dst: usize) -> Vec<usize> {
        RESERVED_REGS
            .iter()
            .copied()
            .filter(|reg| *reg != dst && self.used_reg.is_used(&RegisterMode::Normal(*reg)))
            .collect()
    }

    /// 関数一つ分のアセンブリ言語を生成し始める前に、前の関数の
    /// 状態を全て捨てる。
    ///
    /// 以前は`_start`を最初に生成した後に、この初期化をしていなかった
    /// ため、`_start`の変数(`var_hash_map`)や使用中のレジスタ、
    /// 式の結果のレジスタ(`last_inst_idx`)が次の関数へ持ち越されていた。
    /// (`last_inst_idx`は関数ごとに振り直される式のidで引くので、
    ///  前の関数の同じidの式のレジスタを誤って使うことがあった)
    pub(in crate::compiler::asm_gen) fn reset_fn_state(&mut self) {
        // `data_map`は「IRのノードid → 静的領域のラベル」の対応表で、
        // ノードidは関数ごとに振り直される。前の関数の分が残っていると、
        // `static_label`が先頭から探すため、別の関数の同じidのラベル
        // (`M1`など)を誤って返してしまう。ラベル名の重複は`data_idx`
        // (関数をまたいで増え続ける)で防いでおり、データ領域の
        // テキスト(`data_sec_text`)は`AsmEmitter`が持つので残る。
        self.fn_state = FnState::default();
        self.used_reg.clear();
    }
}
