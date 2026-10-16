//! 変数の遅延復元(退避した変数は、その変数が呼ばれるまで退避したままにする)
//!
//! 関数を呼ぶとき、呼び出しをまたいで使うレジスタは退避する。以前は、変数も
//! 一時的な値と同じく`push`して、呼び出しの直後に`pop`していたため、その変数が
//! 次に使われなくても毎回復元していた。
//!
//! ここでは、変数が入っているレジスタ(`caller_saved`)を、関数のスタックフレームの
//! 専用の領域(`%rbp`相対)へ`mov`で退避し、その変数を参照するノードの生成の直前まで
//! 復元しない。
//!
//! ## 決まり
//! - 退避中のレジスタは`spilled_regs`(レジスタ -> 領域のオフセット)に持つ。
//!   レジスタの中身は壊れているものとして扱い、使用中のままにする(他の値に使わない)。
//! - ノードを生成する前に([`AsmEmitter::prepare_node_spills`])、そのノードが参照する
//!   変数のレジスタが退避中なら復元する。参照する変数は、ノードの引数のIRをたどって
//!   集める。たどれないノード(構造体・バリアントなど)は、全部復元する。
//! - 同じノードの中で関数を呼ぶ場合、そのノードが参照する変数は呼び出しの直後に
//!   復元する(後続のオペランドで使うため)。それ以外の変数だけを退避したままにする。
//! - 制御フローの合流点で状態が食い違わないよう、`Block`・`Jmp`・`ExpectJmp`・
//!   インラインアセンブラの前で、全て復元する。条件ジャンプの条件式の中の呼び出し
//!   (`ExpectJmp`の後)は、退避したままにせず、直後に復元する。
//! - `Ret`の後・`EndScope`で解放されたレジスタの退避は、捨てる(復元しない)。

use super::*;
use std::collections::HashSet;

impl AsmEmitter {
    /// レジスタ`reg`を退避する領域の`%rbp`からのオフセット
    /// (関数ごとにレジスタ1つにつき1回だけ確保し、`stk_use_counter`を進める)
    fn spill_slot(&mut self, reg: RegisterMode) -> usize {
        if let Some(offset) = self.spill_slots.get(&reg) {
            return *offset;
        }
        let offset = self.stk_use_counter.div_ceil(8) * 8 + 8;
        self.stk_use_counter = offset;
        self.spill_slots.insert(reg, offset);
        offset
    }

    /// レジスタ`reg`を領域`slot`へ退避する命令
    fn spill_store_asm(&self, reg: RegisterMode, slot: usize) -> String {
        let slot_ref = self.rbp_ref(slot);
        match reg {
            RegisterMode::Normal(_) => self.mov_line(&slot_ref, &self.reg64(&reg), &Size::DQ, true),
            RegisterMode::Float(_) => {
                let size = self.float_reg_size(reg);
                let opcode = Self::float_move_opcode(&size);
                self.asm_fmt
                    .get_opcode_tmpl(opcode)
                    .replace("{src1}", &self.reg_mode_operand(reg, &size))
                    .replace("{dst}", &slot_ref)
            }
        }
    }

    /// 領域`slot`からレジスタ`reg`へ復元する命令
    fn spill_load_asm(&self, reg: RegisterMode, slot: usize) -> String {
        let slot_ref = self.rbp_ref(slot);
        match reg {
            RegisterMode::Normal(_) => self.mov_line(&self.reg64(&reg), &slot_ref, &Size::DQ, true),
            RegisterMode::Float(_) => {
                let size = self.float_reg_size(reg);
                let opcode = Self::float_move_opcode(&size);
                self.asm_fmt
                    .get_opcode_tmpl(opcode)
                    .replace("{src1}", &slot_ref)
                    .replace("{dst}", &self.reg_mode_operand(reg, &size))
            }
        }
    }

    /// 生きている変数(スコープが終わっていない)で、レジスタ`reg`に置かれているものの名前
    fn live_vars_in_reg(&self, reg: RegisterMode) -> Vec<&str> {
        self.var_hash_map
            .iter()
            .filter(|(name, var)| {
                !var.is_stack
                    && !var.in_mem
                    && var.reg == reg
                    && !self.dead_vars.contains(name.as_str())
            })
            .map(|(name, _)| name.as_str())
            .collect()
    }

    /// 退避しているレジスタ`reg`を復元する(退避していなければ何もしない)
    fn reload_reg(&mut self, reg: RegisterMode) {
        if let Some(slot) = self.spilled_regs.remove(&reg) {
            let asm = self.spill_load_asm(reg, slot);
            self.asm_text.push_str(&asm);
        }
    }

    /// 退避しているレジスタを全て復元する
    pub(super) fn reload_all_spilled(&mut self) {
        let regs: Vec<RegisterMode> = self.spilled_regs.keys().copied().collect();
        for reg in regs {
            self.reload_reg(reg);
        }
    }

    /// `EndScope`などでレジスタが解放された変数の退避を捨てる(復元しない)
    pub(super) fn drop_dead_spills(&mut self) {
        let used = &self.used_reg;
        self.spilled_regs.retain(|reg, _| used.is_used(reg));
    }

    /// ノード`id`が参照する変数の名前を`names`へ集める
    ///
    /// すべてたどれたら`true`、たどれないノードがあったら`false`
    /// (`false`のときは、呼び出し側が全部復元する)
    fn collect_node_refs(
        &self,
        id: usize,
        visited: &mut HashSet<usize>,
        names: &mut Vec<String>,
    ) -> bool {
        let Some(node) = self.curr_inst.get(id) else {
            return true;
        };
        if !visited.insert(id) {
            return true;
        }
        match node {
            // 変数の参照(定義のノードのidが、そのまま変数を指す)
            inst::Inst::Mov {
                name: Some(name), ..
            } => {
                names.push(name.clone());
                true
            }
            inst::Inst::Mov {
                name: None, src, ..
            } => self.collect_node_refs(*src, visited, names),
            inst::Inst::Param(param) => {
                names.push(param.name.clone());
                true
            }
            inst::Inst::Expr(expr) => {
                self.collect_node_refs(expr.ls, visited, names)
                    && self.collect_node_refs(expr.rs, visited, names)
            }
            inst::Inst::Pointer(inner) | inst::Inst::GetAddress(inner) => {
                self.collect_node_refs(*inner, visited, names)
            }
            inst::Inst::InitArr(ids) => ids
                .iter()
                .all(|item| self.collect_node_refs(*item, visited, names)),
            inst::Inst::CallFunc(meta) => meta
                .params
                .iter()
                .all(|param| self.collect_node_refs(*param, visited, names)),
            inst::Inst::RefStruct { src, .. } | inst::Inst::RefStructPtr { src, .. } => {
                names.push(src.clone());
                true
            }
            inst::Inst::InsertArr { name, dst, index } => {
                names.push(name.clone());
                self.collect_node_refs(*dst, visited, names)
                    && self.collect_node_refs(*index, visited, names)
            }
            inst::Inst::AssignVar { name, dst, value } => {
                names.push(name.clone());
                self.collect_node_refs(*dst, visited, names)
                    && self.collect_node_refs(*value, visited, names)
            }
            inst::Inst::Ret(idx) => self.collect_node_refs(*idx, visited, names),
            inst::Inst::Num { .. } | inst::Inst::Float { .. } | inst::Inst::Str { .. } => true,
            _ => false,
        }
    }

    /// ノード`node`を生成する前の、退避した変数の復元
    ///
    /// 制御フローのノード(`Block`/`Jmp`/`ExpectJmp`)とインラインアセンブラの前は
    /// 全て復元する。それ以外は、ノードが参照する変数のレジスタだけを復元し、
    /// `node_vars`に記録する(ノードの中の関数呼び出しが、復元の要否を決めるため)。
    pub(super) fn prepare_node_spills(&mut self, node: &inst::Inst) {
        self.node_vars.clear();
        self.node_vars_all = false;

        let mut names = Vec::new();
        let known = match node {
            inst::Inst::Block(_) | inst::Inst::Jmp(_) | inst::Inst::ExpectJmp(_) => {
                self.reload_all_spilled();
                return;
            }
            inst::Inst::StartScope | inst::Inst::EndScope => return,
            inst::Inst::Comple { .. } => false,
            // 変数の定義は、初期値の式が参照する変数だけを見る
            inst::Inst::Mov { src, .. } => {
                self.collect_node_refs(*src, &mut HashSet::new(), &mut names)
            }
            inst::Inst::Expr(_)
            | inst::Inst::AssignVar { .. }
            | inst::Inst::Ret(_)
            | inst::Inst::CallFunc(_)
            | inst::Inst::InsertArr { .. } => {
                // ノード自身は`visited`に入れず、根の参照を集める
                self.collect_root_refs(node, &mut names)
            }
            inst::Inst::Num { .. }
            | inst::Inst::Float { .. }
            | inst::Inst::Str { .. }
            | inst::Inst::InitArr(_)
            | inst::Inst::Param(_) => return,
            _ => false,
        };

        if !known {
            self.node_vars_all = true;
            self.reload_all_spilled();
            return;
        }
        for name in &names {
            if let Some(reg) = self.var_reg(name) {
                self.reload_reg(reg);
            }
        }
        self.node_vars.extend(names);
    }

    /// ノード自身(根)が参照する変数を集める(自身のidは`visited`に入れない)
    fn collect_root_refs(&self, node: &inst::Inst, names: &mut Vec<String>) -> bool {
        let mut visited = HashSet::new();
        match node {
            inst::Inst::Expr(expr) => {
                self.collect_node_refs(expr.ls, &mut visited, names)
                    && self.collect_node_refs(expr.rs, &mut visited, names)
            }
            inst::Inst::AssignVar { name, dst, value } => {
                names.push(name.clone());
                self.collect_node_refs(*dst, &mut visited, names)
                    && self.collect_node_refs(*value, &mut visited, names)
            }
            inst::Inst::Ret(idx) => self.collect_node_refs(*idx, &mut visited, names),
            inst::Inst::CallFunc(meta) => meta
                .params
                .iter()
                .all(|param| self.collect_node_refs(*param, &mut visited, names)),
            inst::Inst::InsertArr { name, dst, index } => {
                names.push(name.clone());
                self.collect_node_refs(*dst, &mut visited, names)
                    && self.collect_node_refs(*index, &mut visited, names)
            }
            _ => false,
        }
    }

    /// 関数呼び出しの前に、レジスタ`reg`を遅延復元で退避してよいか
    ///
    /// 退避してよいのは、生きている変数が置かれているレジスタだけ
    /// (式の一時的な結果は変数の参照で復元できないので、`push`/`pop`のまま)。
    /// 条件ジャンプの条件式の中・参照を調べられないノードの中では退避しない
    pub(super) fn can_spill_lazily(&self, reg: RegisterMode) -> bool {
        !self.node_vars_all
            && self.reserved_label_name.is_none()
            && !self.live_vars_in_reg(reg).is_empty()
    }

    /// 呼び出しの直後に復元が必要か(今のノードが参照する変数のレジスタか)
    pub(super) fn spill_needed_now(&self, reg: RegisterMode) -> bool {
        self.live_vars_in_reg(reg)
            .iter()
            .any(|name| self.node_vars.contains(*name))
    }

    /// 呼び出しの前に`regs`を領域へ退避する命令を返す
    pub(super) fn spill_store_regs(&mut self, regs: &[RegisterMode]) -> String {
        let mut asm = String::new();
        for reg in regs {
            let slot = self.spill_slot(*reg);
            asm.push_str(&self.spill_store_asm(*reg, slot));
        }
        asm
    }

    /// 呼び出しの後に、`regs`のうち今のノードが参照するものは復元し、
    /// 残りは退避したまま(`spilled_regs`)にする。復元する命令を返す
    pub(super) fn after_call_spills(&mut self, regs: &[RegisterMode]) -> String {
        let mut asm = String::new();
        for reg in regs {
            let slot = self.spill_slot(*reg);
            if self.spill_needed_now(*reg) {
                asm.push_str(&self.spill_load_asm(*reg, slot));
            } else {
                self.spilled_regs.insert(*reg, slot);
            }
        }
        asm
    }
}
