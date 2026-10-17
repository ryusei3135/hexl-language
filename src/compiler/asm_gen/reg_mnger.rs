//! レジスタの使用状況と、変数へのレジスタの割り当てを管理する
//!
//! ## 変数へのレジスタの割り当て
//! 関数のアセンブリを作り始める前に、IRが数えた変数の使用回数
//! (`ir/counter_mnger.rs`の`VarCounter`)をもとに、使用回数が多い変数から
//! 順にレジスタを割り当てておく([`UsedRegManager::allocation_register`])。
//! 変数を定義するとき(`call_func/mem_ir.rs`)は、割り当てられたレジスタが
//! 空いていればそれを使い、空いていなければ空きレジスタを探す。
//!
//! - 割り当てたレジスタは、変数が定義されるまで(`pending`)他の式の一時的な
//!   結果に使われないよう取り置く。
//! - 変数を定義したら(`bind_var`)取り置きをやめ、使用中(`used`)として扱う。
//! - スコープ(反復処理・関数内のブロック)が終わったら、その中で使用中に
//!   なったレジスタを全て解放する([`UsedRegManager::end_scope`])。
//!   解放したレジスタは、他の変数や式が使える。
//!
use std::collections::HashMap;

use crate::compiler::asm_gen::asm_emitter::RESERVED_REGS;
use crate::compiler::ir::counter_mnger;

pub(in crate::compiler::asm_gen) const MARKED_REG: bool = true;
pub(in crate::compiler::asm_gen) const UNMARKED_REG: bool = false;

use super::*;

/// 現在使用中のレジスタを管理する
#[derive(Debug, Clone, Default)]
pub struct UsedRegManager {
    used: Vec<RegisterMode>,
    /// `StartScope`が来た時点で使用中だったレジスタの一覧
    /// (スコープがネストしても対応できるようにスタックで持つ)
    ///
    /// 式の一時的な結果を途中で`release`すると`used`の並びが
    /// ずれるので、長さではなくレジスタ番号そのものを控えておく
    scope_marks: Vec<Vec<RegisterMode>>,
    /// 現在の関数で、変数の名前 -> 割り当てたレジスタ
    allocated: HashMap<String, RegisterMode>,
    /// 割り当て済みだが、まだ変数が定義されていないレジスタ
    /// (他の式の結果に使わせない)
    pending: Vec<RegisterMode>,
    /// 現在の関数で、一度でも使用中にしたレジスタ
    /// (`callee_saved`のうち、退避が必要なものを調べるために使う)
    touched: Vec<RegisterMode>,
    callee_saved: Rc<Vec<usize>>,
    caller_saved: Rc<Vec<usize>>,
    float_callee_saved: Rc<Vec<usize>>,
    float_caller_saved: Rc<Vec<usize>>,
}

impl UsedRegManager {
    pub fn new(
        callee: Rc<Vec<usize>>,
        caller: Rc<Vec<usize>>,
        float_callee: Rc<Vec<usize>>,
        float_caller: Rc<Vec<usize>>,
    ) -> Self {
        Self {
            used: Vec::new(),
            scope_marks: Vec::new(),
            allocated: HashMap::new(),
            pending: Vec::new(),
            touched: Vec::new(),
            // レジスタの番号(`rax`=0, `rcx`=1, `rdx`=2, `rbx`=3, `rsi`=4, `rdi`=5,
            // `r8`〜`r15`=6〜13)
            // - `rbx`と`r12`〜`r15`がSystem V ABIのcallee-saved
            // - `rax`/`rdx`(0/2)は`idiv`などが書き換えるので割り当てない
            callee_saved: callee,
            caller_saved: caller,
            float_callee_saved: float_callee,
            float_caller_saved: float_caller,
        }
    }

    /// IRで数えた変数の使用回数をもとに、レジスタを割り当てる
    ///
    /// 使用回数が多い変数から順に、`callee_saved`、`caller_saved`の順で
    /// 空いているレジスタを割り当てる。レジスタが足りなくなった変数は
    /// 割り当てず、定義するときに空きレジスタを探す。
    ///
    /// ## 引数
    /// - unavailable: 割り当てに使えないレジスタ。この関数自身の引数の
    ///   レジスタ(`%rdi`など。値がまだ入っている)を渡す
    /// - stack_vars: レジスタに置かず、スタックに置く変数の名前
    ///   ([`UsedRegManager::stack_var_names`])。変数の使用回数の表に
    ///   出てきても割り当てずに飛ばし、次の変数にレジスタを割り当てる
    pub fn allocation_register(
        &mut self,
        var_counter: &counter_mnger::VarCounter,
        unavailable: &[RegisterMode],
        stack_vars: &[String],
        body: &[inst::Inst],
    ) {
        self.allocated.clear();
        self.pending.clear();

        let mut normal_candidates = self
            .callee_saved
            .iter()
            .chain(self.caller_saved.iter())
            .copied()
            .filter(|reg| !RESERVED_REGS.contains(reg))
            .map(RegisterMode::Normal)
            .filter(|reg| !unavailable.contains(reg));
        let mut float_candidates = self
            .float_callee_saved
            .iter()
            .chain(self.float_caller_saved.iter())
            .copied()
            .map(RegisterMode::Float)
            .filter(|reg| !unavailable.contains(reg));

        for (name, count) in var_counter.vars_by_priority() {
            // 使用回数が0回の変数は生成しないので、レジスタも割り当てない
            if Self::count_is_zero(count) {
                continue;
            }
            // スタックに置く変数(`s: Name = Name { .. }`など)は、
            // レジスタを持たない(`VarIndexInfo::new_stack`)ので割り当てない。
            // 割り当ててしまうと、変数が定義されても`bind_var`が呼ばれず、
            // 取り置き(`pending`)のレジスタが解放されないまま無駄になる
            if stack_vars.iter().any(|stack_var| stack_var == name) {
                continue;
            }
            let is_float = body.iter().any(|node| {
                matches!(
                    node,
                    inst::Inst::Mov {
                        name: Some(var_name),
                        size,
                        ..
                    } if var_name == name && size.is_float()
                )
            });
            let reg = if is_float {
                float_candidates.next()
            } else {
                normal_candidates.next()
            };
            let Some(reg) = reg else {
                continue;
            };
            self.allocated.insert(name.to_string(), reg);
            self.pending.push(reg);
        }
    }

    /// 使用回数(`vars_by_priority`が返す回数。値でも参照でもよい)を`usize`で返す
    fn count_value<C: std::borrow::Borrow<usize>>(count: C) -> usize {
        *count.borrow()
    }

    /// 使用回数が0回か
    fn count_is_zero<C: std::borrow::Borrow<usize>>(count: C) -> bool {
        Self::count_value(count) == 0
    }

    /// 生成しない変数を定義する`Inst::Mov`のノードid(`body`の添字)を集める
    ///
    /// ## 生成しない変数
    /// 1. 使用回数が0回の変数
    /// 2. 生成しない変数の初期値の中で参照される変数のうち、
    ///    使用回数のすべてがその初期値の中にあるもの。
    ///
    /// ## 対象外
    /// - 初期値の式に関数の呼び出し・代入・inlineアセンブラがある変数:
    ///   生成しないと副作用が消えてしまうため
    /// - 同じ名前の変数への再代入(`Inst::AssignVar`)がある変数:
    ///   再代入のために変数の情報が必要なため
    ///
    /// 使用回数は、`var_counter`に出てこない変数を0回として扱う
    ///
    /// 戻り値には、生成しない変数の`Inst::Mov`に加えて、その初期値の式だけを
    /// 構成するノード(`Inst::Expr`・`Inst::Float`・`Inst::Str`)のidも含める
    /// (`gen_node_asm`はこれらも生成しない。残すと、式が未登録の変数を参照したり、
    /// 使われない式の命令・定数が出力されたりする)
    pub(in crate::compiler::asm_gen) fn unused_var_nodes(
        var_counter: &counter_mnger::VarCounter,
        body: &[inst::Inst],
    ) -> std::collections::HashSet<usize> {
        use std::collections::{HashMap, HashSet};

        let mut counts: HashMap<String, usize> = HashMap::new();
        for (name, count) in var_counter.vars_by_priority() {
            *counts.entry(name.to_string()).or_insert(0) += Self::count_value(count);
        }
        let assigned: HashSet<&str> = body
            .iter()
            .filter_map(|node| match node {
                inst::Inst::AssignVar { name, .. } => Some(name.as_str()),
                _ => None,
            })
            .collect();

        let mut removed: HashSet<usize> = HashSet::new();
        // 生成しない変数の式の中で使われている回数(変数の名前ごと)
        let mut dead_uses: HashMap<String, usize> = HashMap::new();

        loop {
            let mut changed = false;
            for (idx, node) in body.iter().enumerate() {
                let inst::Inst::Mov {
                    name: Some(name),
                    src,
                    ..
                } = node
                else {
                    continue;
                };
                if removed.contains(&idx) || assigned.contains(name.as_str()) {
                    continue;
                }
                // 生成する(生成しない式の外で使われている)変数が残っている
                let uses = counts.get(name.as_str()).copied().unwrap_or(0);
                if uses > dead_uses.get(name.as_str()).copied().unwrap_or(0) {
                    continue;
                }
                let mut refs = Vec::new();
                let mut dead_nodes = Vec::new();
                if !Self::collect_pure_expr_vars(
                    body,
                    *src,
                    &mut HashSet::new(),
                    &mut refs,
                    &mut dead_nodes,
                ) {
                    continue;
                }
                removed.insert(idx);
                removed.extend(dead_nodes);
                for var in refs {
                    *dead_uses.entry(var).or_insert(0) += 1;
                }
                changed = true;
            }
            if !changed {
                break;
            }
        }
        removed
    }

    /// 式`id`が副作用の無い式(関数の呼び出し・代入・inlineアセンブラを
    /// 含まない)なら`true`を返し、式の中で使われている変数(`Inst::Mov`のノード)
    /// の名前を`vars`へ追加する
    ///
    /// 式を構成するノード(`Inst::Expr`・`Inst::Float`・`Inst::Str`)のidは
    /// `nodes`へ追加する(変数が生成されないとき、これらも生成しない)。
    ///
    /// 同じノードに複数回たどり着いても、1回だけ数える(`visited`)。
    /// 数え漏らす分には、変数を生成する側(安全側)に倒れるだけなので、
    /// 数え過ぎないことを優先している
    fn collect_pure_expr_vars(
        body: &[inst::Inst],
        id: usize,
        visited: &mut std::collections::HashSet<usize>,
        vars: &mut Vec<String>,
        nodes: &mut Vec<usize>,
    ) -> bool {
        if !visited.insert(id) {
            return true;
        }
        let Some(node) = body.get(id) else {
            return true;
        };
        match node {
            // 変数の使用(中身の式はたどらない)
            inst::Inst::Mov {
                name: Some(name), ..
            } => {
                vars.push(name.clone());
                true
            }
            // 式(`fv + f2`など)。オペランドも副作用が無ければ、式のノードごと
            // 生成しない(`nodes`に加える)
            inst::Inst::Expr(expr) => {
                if matches!(expr.kind, inst::ExprKind::Inc | inst::ExprKind::Dec) {
                    return false;
                }
                if !Self::collect_pure_expr_vars(body, expr.ls, visited, vars, nodes)
                    || !Self::collect_pure_expr_vars(body, expr.rs, visited, vars, nodes)
                {
                    return false;
                }
                nodes.push(id);
                true
            }
            // キャスト: 元の値が副作用の無い式なら、キャストのノードごと生成しない
            inst::Inst::Cast { src, .. } => {
                if !Self::collect_pure_expr_vars(body, *src, visited, vars, nodes) {
                    return false;
                }
                nodes.push(id);
                true
            }
            // 浮動小数点・文字列のリテラル(定数を`.data`へ出力するノード)
            inst::Inst::Float { .. } | inst::Inst::Str { .. } => {
                nodes.push(id);
                true
            }
            inst::Inst::GetAddress(inner) | inst::Inst::Pointer(inner) => {
                Self::collect_pure_expr_vars(body, *inner, visited, vars, nodes)
            }
            inst::Inst::InitArr(ids) => ids
                .iter()
                .all(|item| Self::collect_pure_expr_vars(body, *item, visited, vars, nodes)),
            // 副作用がある
            inst::Inst::CallFunc(..) | inst::Inst::AssignVar { .. } | inst::Inst::Comple { .. } => {
                false
            }
            _ => true,
        }
    }

    /// 関数の本体のIRから、レジスタに置かずスタックに置く変数の名前を集める
    ///
    /// ポインタでない構造体の変数(`s: Name = Name { .. }`)は、`%rbp`からの
    /// オフセットで保存する(`call_func/mem_ir.rs`の`mov_value_ir`)。
    /// 同じ名前が別のスコープで構造体以外として定義されていても、
    /// 名前ごとに割り当てるので、1つでも構造体の定義があれば対象にする
    pub(in crate::compiler::asm_gen) fn stack_var_names(body: &[inst::Inst]) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for node in body {
            if let inst::Inst::Mov {
                name: Some(name),
                size: Size::Struct(..),
                ..
            } = node
            {
                if !names.contains(name) {
                    names.push(name.clone());
                }
            }
        }
        names
    }

    /// 変数`name`に割り当てたレジスタ(割り当てていなければ`None`)
    pub(in crate::compiler::asm_gen) fn allocated_reg(&self, name: &str) -> Option<RegisterMode> {
        self.allocated.get(name).copied()
    }

    /// 変数`name`に割り当てたレジスタを取り消す(変数を生成しないことにした場合に、
    /// 取り置いたレジスタを他の変数や式が使えるようにする)
    pub(in crate::compiler::asm_gen) fn release_allocation(&mut self, name: &str) {
        if let Some(reg) = self.allocated.remove(name) {
            self.pending.retain(|r| *r != reg);
        }
    }

    /// `reg`が、まだ定義されていない変数のために取り置かれているか
    /// (他の式の一時的な結果に使ってはいけない)
    pub(in crate::compiler::asm_gen) fn is_reserved(&self, reg: RegisterMode) -> bool {
        self.pending.contains(&reg)
    }

    /// 変数`name`を定義し、レジスタ`reg`に置いたことを記録する
    /// (割り当てたレジスタなら、取り置きをやめて使用中として扱う)
    pub(in crate::compiler::asm_gen) fn bind_var(&mut self, name: &str, reg: &RegisterMode) {
        if self.allocated.get(name) == Some(&reg) {
            self.pending.retain(|r| r != reg);
        }
        self.mark_used(&reg);
    }

    /// 指定したレジスタを使用中として記録する
    /// Some(..)の場合使われていないレジスタを返す
    pub(in crate::compiler::asm_gen) fn mark_used(&mut self, reg: &RegisterMode) -> bool {
        if !self.touched.contains(&reg) {
            self.touched.push(reg.clone());
        }
        if !self.used.contains(&reg) {
            self.used.push(reg.clone());
            MARKED_REG
        } else {
            UNMARKED_REG
        }
    }

    /// 現在使用中のレジスタ番号を、使用され始めた順(昇順)に取得する
    pub(in crate::compiler::asm_gen) fn used_regs(&self) -> Vec<RegisterMode> {
        let mut regs = self.used.clone();
        regs.sort();
        regs
    }

    /// 現在の関数で使ったレジスタのうち、`callee_saved`に当たるもの
    /// (関数の入口で退避し、出口で戻す必要がある。昇順)
    pub(in crate::compiler::asm_gen) fn callee_regs_to_save(&self) -> Vec<RegisterMode> {
        let mut regs: Vec<RegisterMode> = self
            .touched
            .iter()
            .copied()
            .filter(|reg| match reg {
                RegisterMode::Float(r) => self.float_callee_saved.contains(r),
                RegisterMode::Normal(r) => self.callee_saved.contains(r),
            })
            .collect();
        regs.sort();
        regs
    }

    /// 記録している使用中のレジスタの情報を全て消去する
    /// (関数一つ分のアセンブリ言語の生成が終わった際などに使う)
    pub(in crate::compiler::asm_gen) fn clear(&mut self) {
        self.used.clear();
        self.scope_marks.clear();
        self.allocated.clear();
        self.pending.clear();
        self.touched.clear();
    }

    /// スコープの開始を記録する
    /// これ以降に`mark_used`で登録されたレジスタが、
    /// 対応する`end_scope`で解放される対象になる
    pub fn start_scope(&mut self) {
        self.scope_marks.push(self.used.clone());
    }

    /// 直近の`start_scope`以降に登録されたレジスタを全て解放し、
    /// 解放したレジスタ番号を返す
    /// (`start_scope`より前から使用中のレジスタは残る)
    ///
    /// スコープの中で定義された変数のレジスタもここで解放されるので、
    /// 以降は他の変数や式が使える。変数の名前と割り当ては残るので、
    /// 同じ名前の変数を別のスコープで定義するときは、空いていれば
    /// 同じレジスタを使う。
    pub(in crate::compiler::asm_gen) fn end_scope(&mut self) -> Vec<RegisterMode> {
        let outer = self
            .scope_marks
            .pop()
            .expect("StartScopeに対応しないEndScopeです");
        let released: Vec<RegisterMode> = self
            .used
            .iter()
            .copied()
            .filter(|reg| !outer.contains(reg))
            .collect();
        self.used.retain(|reg| outer.contains(reg));
        released
    }

    /// 指定したレジスタを、使用中の記録から外す
    /// (式の一時的な結果を、使い終わった時点で解放するために使う)
    pub(in crate::compiler::asm_gen) fn release(&mut self, reg: &RegisterMode) {
        self.used.retain(|r| r != reg);
    }

    /// 指定したレジスタが使用中かどうか
    pub(in crate::compiler::asm_gen) fn is_used(&self, reg: &RegisterMode) -> bool {
        self.used.contains(&reg)
    }

    /// `regs`の中で、現在使用中でないレジスタのうち最小の番号を返す
    /// (`EndScope`で解放したレジスタから、次に使うレジスタを選ぶために使う)
    pub(in crate::compiler::asm_gen) fn lowest_free<'a>(&self, regs: &'a [RegisterMode]) -> Option<&'a RegisterMode> {
        regs.iter()
            .filter(|reg| !self.is_used(reg))
            .min_by_key(|reg| match reg {
                RegisterMode::Normal(index) | RegisterMode::Float(index) => *index,
            })
    }

    /// 指定したレジスタを使用中として登録し直す
    /// (スコープ終了後も生きている変数のレジスタを残すために使う)
    pub(in crate::compiler::asm_gen) fn restore(&mut self, regs: &[RegisterMode]) {
        regs.iter().for_each(|reg| {
            self.mark_used(reg);
        });
    }
}

#[cfg(test)]
mod unused_var_tests {
    use super::*;

    fn mov(name: &str, dst: usize, src: usize) -> inst::Inst {
        inst::Inst::Mov {
            name: Some(name.to_string()),
            size: Size::F32,
            dst,
            src,
        }
    }

    fn add(dst: usize, ls: usize, rs: usize) -> inst::Inst {
        inst::Inst::Expr(inst::ExprInst {
            dst,
            ls,
            rs,
            kind: inst::ExprKind::Add,
        })
    }

    fn inc(dst: usize, value: usize) -> inst::Inst {
        inst::Inst::Expr(inst::ExprInst {
            dst,
            ls: value,
            rs: value,
            kind: inst::ExprKind::Inc,
        })
    }

    /// `fv3: float = fv + f2`(`fv3`は未使用、`fv`/`f2`は`fv3`の式でのみ使用)
    #[test]
    fn removes_variables_used_only_by_the_expression_of_an_unused_variable() {
        let body = vec![
            mov("fv", 0, 4),
            mov("f2", 1, 5),
            add(2, 0, 1),
            mov("fv3", 3, 2),
        ];
        let mut var_counter = counter_mnger::VarCounter::new();
        var_counter.use_var("fv");
        var_counter.use_var("f2");

        let removed = UsedRegManager::unused_var_nodes(&var_counter, &body);
        // fv, f2, 式, fv3 のすべてを生成しない
        assert!(removed.contains(&0) && removed.contains(&1));
        assert!(removed.contains(&2) && removed.contains(&3));
    }

    /// `fv`が式の外(`g: float = fv`など)でも使われているなら、`fv`は残す
    #[test]
    fn keeps_a_variable_that_is_also_used_outside_the_unused_expression() {
        let body = vec![
            mov("fv", 0, 4),
            mov("f2", 1, 5),
            add(2, 0, 1),
            mov("fv3", 3, 2),
        ];
        let mut var_counter = counter_mnger::VarCounter::new();
        var_counter.use_var("fv");
        var_counter.use_var("fv");
        var_counter.use_var("f2");

        let removed = UsedRegManager::unused_var_nodes(&var_counter, &body);
        assert!(!removed.contains(&0));
        assert!(removed.contains(&1) && removed.contains(&3));
    }

    /// 生成する変数(`fv3`が使われている)の式や変数は残す
    #[test]
    fn keeps_everything_when_the_variable_is_used() {
        let body = vec![
            mov("fv", 0, 4),
            mov("f2", 1, 5),
            add(2, 0, 1),
            mov("fv3", 3, 2),
        ];
        let mut var_counter = counter_mnger::VarCounter::new();
        var_counter.use_var("fv");
        var_counter.use_var("f2");
        var_counter.use_var("fv3");

        assert!(UsedRegManager::unused_var_nodes(&var_counter, &body).is_empty());
    }

    #[test]
    fn keeps_increment_side_effects_inside_unused_initializers() {
        let body = vec![mov("value", 0, 4), inc(1, 0), mov("unused", 2, 1)];
        let mut var_counter = counter_mnger::VarCounter::new();
        var_counter.use_var("value");

        let removed = UsedRegManager::unused_var_nodes(&var_counter, &body);
        assert!(removed.is_empty());
    }
}

#[cfg(test)]
mod stack_var_tests {
    use super::*;

    fn manager() -> UsedRegManager {
        // callee_saved: rbx(3), r12(12) / caller_saved: rcx(1)
        UsedRegManager::new(
            Rc::new(vec![3, 12]),
            Rc::new(vec![1]),
            Rc::new(vec![]),
            Rc::new(vec![0, 1, 2]),
        )
    }

    #[test]
    fn lowest_free_returns_the_smallest_register_index() {
        let regs = manager();
        let candidates = [
            RegisterMode::Normal(8),
            RegisterMode::Float(2),
            RegisterMode::Normal(5),
        ];

        assert_eq!(
            regs.lowest_free(&candidates),
            Some(RegisterMode::Float(2)).as_ref()
        );
    }

    #[test]
    fn stack_var_is_skipped_and_next_var_gets_the_register() {
        let mut counter = counter_mnger::VarCounter::new();
        // `s`(構造体)が最も多く使われ、次に`a`、`b`
        for _ in 0..3 {
            counter.use_var("s");
        }
        counter.use_var("a");
        counter.use_var("a");
        counter.use_var("b");

        let mut regs = manager();
        regs.allocation_register(&counter, &[], &["s".to_string()], &[]);

        // `s`には割り当てず、取り置きもしない
        assert_eq!(regs.allocated_reg("s"), None);
        // 次の変数に、先頭のレジスタから順に割り当てる
        assert_eq!(regs.allocated_reg("a"), Some(RegisterMode::Normal(3)));
        assert_eq!(regs.allocated_reg("b"), Some(RegisterMode::Normal(12)));
        assert!(
            regs.is_reserved(RegisterMode::Normal(3)) && regs.is_reserved(RegisterMode::Normal(12))
        );
    }

    #[test]
    fn without_stack_vars_the_most_used_var_gets_the_first_register() {
        let mut counter = counter_mnger::VarCounter::new();
        counter.use_var("s");
        counter.use_var("s");
        counter.use_var("a");

        let mut regs = manager();
        regs.allocation_register(&counter, &[], &[], &[]);
        assert_eq!(regs.allocated_reg("s"), Some(RegisterMode::Normal(3)));
        assert_eq!(regs.allocated_reg("a"), Some(RegisterMode::Normal(12)));
    }

    #[test]
    fn allocation_register_uses_float_registers_for_float_variables() {
        let mut counter = counter_mnger::VarCounter::new();
        counter.use_var("i");
        counter.use_var("i");
        counter.use_var("f");
        counter.use_var("f");
        counter.use_var("f");

        let body = vec![
            inst::Inst::Mov {
                name: Some("i".to_string()),
                size: Size::DD,
                dst: 0,
                src: 1,
            },
            inst::Inst::Mov {
                name: Some("f".to_string()),
                size: Size::F32,
                dst: 2,
                src: 3,
            },
        ];

        let mut regs = manager();
        regs.allocation_register(&counter, &[], &[], &body);

        assert_eq!(regs.allocated_reg("f"), Some(RegisterMode::Float(0)));
        assert_eq!(regs.allocated_reg("i"), Some(RegisterMode::Normal(3)));
        assert!(regs.is_reserved(RegisterMode::Float(0)));
        assert!(regs.is_reserved(RegisterMode::Normal(3)));
    }

    #[test]
    fn stack_var_names_collects_struct_movs_only() {
        let body = vec![
            inst::Inst::Mov {
                name: Some("s".to_string()),
                size: Size::Struct(Vec::new()),
                dst: 0,
                src: 1,
            },
            inst::Inst::Mov {
                name: Some("a".to_string()),
                size: Size::DD,
                dst: 2,
                src: 3,
            },
            inst::Inst::Mov {
                name: None,
                size: Size::Struct(Vec::new()),
                dst: 4,
                src: 5,
            },
        ];
        assert_eq!(
            UsedRegManager::stack_var_names(&body),
            vec!["s".to_string()]
        );
    }
}
