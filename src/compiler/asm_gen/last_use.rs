//! 変数の最後の使用(`last use`)のあとで、その変数のレジスタを解放する
//!
//! 変数のレジスタは、以前はスコープが終わる(`EndScope`、関数なら最後)まで使用中のままだった。
//! そのため、`_start`のように関数全体が1つのスコープだと、1回しか使わない変数
//! (`msg: char* = "..."`など)のレジスタが、その後の関数呼び出しのたびに
//! 退避・復元され続けていた(`%rcx`が`pushq %rcx`/`popq %rcx`され続けるなど)。
//!
//! ## 最後の使用の決め方([`AsmEmitter::var_last_uses`])
//! 1. IRの全てのノードを根にして、そのノードが(オペランドをたどって)参照する変数を集める。
//!    根のidが最大のものが、その変数の最後の使用になる。
//!    オペランドは根より前のノードなので、関数の呼び出しなど、値を使う側のノードのidが採用される。
//! 2. 変数の定義より前から続く反復処理の中で最後に使っている場合、反復処理の
//!    先頭へ戻る`Jmp`の位置まで延ばす(次の周回でまた使うため)。
//!
//! ## 解放([`AsmEmitter::release_dead_vars`])
//! 最後の使用のノードを生成したら、変数を`dead_vars`に入れ、他の変数も式の結果も
//! 持っていなければレジスタを使用中から外す。レジスタを持たない変数(スタック・
//! メモリ上の実体・静的領域)は対象外。引数(`Inst::Param`)も対象外。

use super::*;
use std::collections::HashSet;

impl AsmEmitter {
    /// ノード`id`が参照する変数の名前を`names`へ集める
    ///
    /// `is_root`が`true`のノード(IRのノード自身)が変数の定義なら、
    /// 初期値(`src`)の式も参照として数える。たどった先の変数の参照
    /// (`Inst::Mov`)は、その変数の使用1回として数えて、中の式はたどらない
    fn node_var_refs(
        insts: &[inst::Inst],
        id: usize,
        is_root: bool,
        visited: &mut HashSet<usize>,
        names: &mut Vec<String>,
    ) {
        let Some(node) = insts.get(id) else {
            return;
        };
        if !visited.insert(id) {
            return;
        }
        let walk = |ids: &[usize], names: &mut Vec<String>, visited: &mut HashSet<usize>| {
            for item in ids {
                Self::node_var_refs(insts, *item, false, visited, names);
            }
        };
        match node {
            inst::Inst::Mov {
                name: Some(name),
                src,
                ..
            } => {
                names.push(name.clone());
                if is_root {
                    walk(&[*src], names, visited);
                }
            }
            inst::Inst::Mov {
                name: None, src, ..
            } => walk(&[*src], names, visited),
            inst::Inst::Param(param) => names.push(param.name.clone()),
            inst::Inst::Expr(expr) => walk(&[expr.ls, expr.rs], names, visited),
            inst::Inst::Pointer(inner) | inst::Inst::GetAddress(inner) => {
                walk(&[*inner], names, visited)
            }
            inst::Inst::Cast { src, .. } => walk(&[*src], names, visited),
            inst::Inst::InitArr(ids) => walk(ids, names, visited),
            inst::Inst::CallFunc(meta) => walk(&meta.params, names, visited),
            inst::Inst::RefStruct { src, .. } | inst::Inst::RefStructPtr { src, .. } => {
                names.push(src.clone())
            }
            inst::Inst::InsertArr { name, dst, index } => {
                names.push(name.clone());
                walk(&[*dst, *index], names, visited);
            }
            inst::Inst::AssignVar { name, dst, value } => {
                names.push(name.clone());
                walk(&[*dst, *value], names, visited);
            }
            inst::Inst::Ret(idx) => walk(&[*idx], names, visited),
            inst::Inst::Comple { lines, .. } => {
                for (_, ids) in lines {
                    walk(ids, names, visited);
                }
            }
            inst::Inst::Struct { mem, .. } => {
                for member in mem {
                    Self::member_var_refs(insts, member, visited, names);
                }
            }
            inst::Inst::Variant { value, .. } => {
                if let Some(member) = value {
                    Self::member_var_refs(insts, member, visited, names);
                }
            }
            inst::Inst::MemoryValue(member) => Self::member_var_refs(insts, member, visited, names),
            _ => {}
        }
    }

    /// `MemoryInst`が参照する変数の名前を集める
    fn member_var_refs(
        insts: &[inst::Inst],
        member: &inst::MemoryInst,
        visited: &mut HashSet<usize>,
        names: &mut Vec<String>,
    ) {
        match member {
            inst::MemoryInst::Member {
                parent, value_idx, ..
            } => {
                names.push(parent.clone());
                Self::node_var_refs(insts, *value_idx, false, visited, names);
            }
            inst::MemoryInst::Memory { src, dst, .. } => {
                for id in src.iter().chain(std::iter::once(dst)) {
                    Self::node_var_refs(insts, *id, false, visited, names);
                }
            }
            inst::MemoryInst::Byte(_) => {}
        }
    }

    /// 関数のIR(`insts`)から、変数の最後の使用のノードのidを、
    /// 「ノードのid -> そこで最後に使い終わる変数の名前」で返す
    ///
    /// 対象は`Inst::Mov`で定義されたローカル変数だけ(引数は含めない)
    pub(in crate::compiler::asm_gen) fn var_last_uses(
        insts: &[inst::Inst],
    ) -> HashMap<usize, Vec<String>> {
        // 変数の最初の定義
        let mut first_def: HashMap<&str, usize> = HashMap::new();
        for (idx, node) in insts.iter().enumerate() {
            if let inst::Inst::Mov {
                name: Some(name), ..
            } = node
            {
                first_def.entry(name.as_str()).or_insert(idx);
            }
        }

        // 反復処理の範囲: 先頭(`Block(name)`)から、先頭へ戻る最後の`Jmp(name)`まで
        let mut loops: Vec<(usize, usize)> = Vec::new();
        for (start_idx, node) in insts.iter().enumerate() {
            let inst::Inst::Block(label) = node else {
                continue;
            };
            let back = insts
                .iter()
                .enumerate()
                .skip(start_idx + 1)
                .filter(|(_, n)| matches!(n, inst::Inst::Jmp(name) if name == label))
                .map(|(idx, _)| idx)
                .last();
            if let Some(end_idx) = back {
                loops.push((start_idx, end_idx));
            }
        }

        let mut last_use: HashMap<String, usize> = HashMap::new();
        for id in 0..insts.len() {
            let mut names = Vec::new();
            Self::node_var_refs(insts, id, true, &mut HashSet::new(), &mut names);
            for name in names {
                if first_def.contains_key(name.as_str()) {
                    let entry = last_use.entry(name).or_insert(id);
                    *entry = (*entry).max(id);
                }
            }
        }

        let mut by_node: HashMap<usize, Vec<String>> = HashMap::new();
        for (name, mut last) in last_use {
            let def = first_def[name.as_str()];
            // 定義より前から続く反復処理の中で使っているなら、次の周回でも使う
            loop {
                let extended = loops
                    .iter()
                    .filter(|(start, end)| def < *start && *start <= last && last <= *end)
                    .map(|(_, end)| *end)
                    .max();
                match extended {
                    Some(end) if end > last => last = end,
                    _ => break,
                }
            }
            by_node.entry(last).or_default().push(name);
        }
        by_node
    }

    /// ノード`node_idx`を生成し終えた時点で使い終わる変数のレジスタを解放する
    /// (`last_uses`は[`AsmEmitter::var_last_uses`]の結果)
    pub(in crate::compiler::asm_gen) fn release_dead_vars(
        &mut self,
        node_idx: usize,
        last_uses: &HashMap<usize, Vec<String>>,
    ) {
        let Some(names) = last_uses.get(&node_idx) else {
            return;
        };
        let mut released_any = false;
        for name in names {
            // 定義されなかった変数(生成しない変数など)に取り置いたレジスタも戻す
            self.used_reg.release_allocation(name);
            let Some(var) = self.find_var_info(name) else {
                continue;
            };
            if self.fn_state.dead_vars.contains(name) || !var.holds_reg() {
                continue;
            }
            let reg = var.reg;
            self.fn_state.dead_vars.insert(name.clone());
            // 別の変数や、まだ使う式の結果がそのレジスタを持っていれば残す
            let held_by_expr = self.fn_state.last_inst_idx.iter().any(|(_, r)| *r == reg);
            if !self.is_reg_held_by_var(reg) && !held_by_expr {
                self.used_reg.release(&reg);
                released_any = true;
            }
        }
        if released_any {
            // 解放したレジスタの退避は、復元せずに捨てる
            self.drop_dead_spills();
        }
    }

    /// ノードが持つ、オペランド(他のノードのid)の一覧
    fn operand_ids(node: &inst::Inst) -> Vec<usize> {
        fn member_ids(member: &inst::MemoryInst, ids: &mut Vec<usize>) {
            match member {
                inst::MemoryInst::Member { value_idx, .. } => ids.push(*value_idx),
                inst::MemoryInst::Memory { src, dst, .. } => {
                    ids.extend(src.iter().copied());
                    ids.push(*dst);
                }
                inst::MemoryInst::Byte(_) => {}
            }
        }
        let mut ids = Vec::new();
        match node {
            inst::Inst::Expr(expr) => ids.extend([expr.ls, expr.rs]),
            inst::Inst::Cast { src, .. } | inst::Inst::Mov { src, .. } => ids.push(*src),
            inst::Inst::Pointer(inner) | inst::Inst::GetAddress(inner) => ids.push(*inner),
            inst::Inst::InitArr(items) => ids.extend(items.iter().copied()),
            inst::Inst::CallFunc(meta) => ids.extend(meta.params.iter().copied()),
            inst::Inst::InsertArr { dst, index, .. } => ids.extend([*dst, *index]),
            inst::Inst::AssignVar { dst, value, .. } => ids.extend([*dst, *value]),
            inst::Inst::Ret(idx) => ids.push(*idx),
            inst::Inst::Comple { lines, .. } => lines
                .iter()
                .for_each(|(_, items)| ids.extend(items.iter().copied())),
            inst::Inst::Struct { mem, .. } => mem.iter().for_each(|m| member_ids(m, &mut ids)),
            inst::Inst::Variant { value, .. } => {
                if let Some(m) = value {
                    member_ids(m, &mut ids)
                }
            }
            inst::Inst::MemoryValue(member) => member_ids(member, &mut ids),
            _ => {}
        }
        ids
    }

    /// 式のノード`idx`の結果を、後ろのノードがオペランドとして使うか
    ///
    /// 使われない式は、`if`/`match`/`loop`の条件の一番外側の式
    /// (結果を条件ジャンプで使う)か、結果を捨てる式
    pub(in crate::compiler::asm_gen) fn is_expr_consumed(&self, idx: usize) -> bool {
        self.fn_state
            .curr_inst
            .iter()
            .skip(idx + 1)
            .any(|node| Self::operand_ids(node).contains(&idx))
    }
}
