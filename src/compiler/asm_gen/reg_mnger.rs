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

pub(in crate::compiler::asm_gen) 
const MARKED_REG: bool = true;
pub(in crate::compiler::asm_gen) 
const UNMARKED_REG: bool = false;

use super::*;

/// 現在使用中のレジスタを管理する
#[derive(Debug, Clone, Default)]
pub struct UsedRegManager {
    used: Vec<usize>,
    /// `StartScope`が来た時点で使用中だったレジスタの一覧
    /// (スコープがネストしても対応できるようにスタックで持つ)
    ///
    /// 式の一時的な結果を途中で`release`すると`used`の並びが
    /// ずれるので、長さではなくレジスタ番号そのものを控えておく
    scope_marks: Vec<Vec<usize>>,
    /// 現在の関数で、変数の名前 -> 割り当てたレジスタ
    allocated: HashMap<String, usize>,
    /// 割り当て済みだが、まだ変数が定義されていないレジスタ
    /// (他の式の結果に使わせない)
    pending: Vec<usize>,
    /// 現在の関数で、一度でも使用中にしたレジスタ
    /// (`callee_saved`のうち、退避が必要なものを調べるために使う)
    touched: Vec<usize>,
    callee_saved: Rc<Vec<usize>>,
    caller_saved: Rc<Vec<usize>>,
}

impl UsedRegManager {
    pub fn new(callee: Rc<Vec<usize>>, caller: Rc<Vec<usize>>) -> Self {
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
    pub fn allocation_register(
        &mut self,
        var_counter: &counter_mnger::VarCounter,
        unavailable: &[usize],
    ) {
        self.allocated.clear();
        self.pending.clear();

        let mut candidates = self.callee_saved
            .iter()
            .chain(self.caller_saved.iter())
            .copied()
            .filter(|reg| !RESERVED_REGS.contains(reg) && !unavailable.contains(reg));

        for (name, _count) in var_counter.vars_by_priority() {
            let Some(reg) = candidates.next() else {
                break;
            };
            self.allocated.insert(name.to_string(), reg);
            self.pending.push(reg);
        }
    }

    /// 変数`name`に割り当てたレジスタ(割り当てていなければ`None`)
    pub fn allocated_reg(&self, name: &str) -> Option<usize> {
        self.allocated.get(name).copied()
    }

    /// `reg`が、まだ定義されていない変数のために取り置かれているか
    /// (他の式の一時的な結果に使ってはいけない)
    pub fn is_reserved(&self, reg: usize) -> bool {
        self.pending.contains(&reg)
    }

    /// 変数`name`を定義し、レジスタ`reg`に置いたことを記録する
    /// (割り当てたレジスタなら、取り置きをやめて使用中として扱う)
    pub fn bind_var(&mut self, name: &str, reg: usize) {
        if self.allocated.get(name) == Some(&reg) {
            self.pending.retain(|r| *r != reg);
        }
        self.mark_used(reg);
    }

    /// 指定したレジスタを使用中として記録する
    /// Some(..)の場合使われていないレジスタを返す
    pub fn mark_used(&mut self, reg: usize) -> bool {
        if !self.touched.contains(&reg) {
            self.touched.push(reg);
        }
        if !self.used.contains(&reg) {
            self.used.push(reg);
            MARKED_REG
        } else {
            UNMARKED_REG
        }
    }

    /// 現在使用中のレジスタ番号を、使用され始めた順(昇順)に取得する
    pub fn used_regs(&self) -> Vec<usize> {
        let mut regs = self.used.clone();
        regs.sort();
        regs
    }

    /// 現在の関数で使ったレジスタのうち、`callee_saved`に当たるもの
    /// (関数の入口で退避し、出口で戻す必要がある。昇順)
    pub fn callee_regs_to_save(&self) -> Vec<usize> {
        let mut regs: Vec<usize> = self
            .touched
            .iter()
            .copied()
            .filter(|reg| self.callee_saved.contains(reg))
            .collect();
        regs.sort();
        regs
    }

    /// 記録している使用中のレジスタの情報を全て消去する
    /// (関数一つ分のアセンブリ言語の生成が終わった際などに使う)
    pub fn clear(&mut self) {
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
    pub fn end_scope(&mut self) -> Vec<usize> {
        let outer = self
            .scope_marks
            .pop()
            .expect("StartScopeに対応しないEndScopeです");
        let released: Vec<usize> = self
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
    pub fn release(&mut self, reg: usize) {
        self.used.retain(|r| *r != reg);
    }

    /// 指定したレジスタが使用中かどうか
    pub fn is_used(&self, reg: usize) -> bool {
        self.used.contains(&reg)
    }

    /// `regs`の中で、現在使用中でないレジスタのうち最小の番号を返す
    /// (`EndScope`で解放したレジスタから、次に使うレジスタを選ぶために使う)
    pub fn lowest_free(&self, regs: &[usize]) -> Option<usize> {
        regs.iter().copied().filter(|r| !self.is_used(*r)).min()
    }

    /// 指定したレジスタを使用中として登録し直す
    /// (スコープ終了後も生きている変数のレジスタを残すために使う)
    pub fn restore(&mut self, regs: &[usize]) {
        for reg in regs {
            self.mark_used(*reg);
        }
    }
}