


pub(in crate::asm_gen) const MARKED_REG: bool = true;
pub(in crate::asm_gen) const UNMARKED_REG: bool = false;

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
}

impl UsedRegManager {
    pub fn new() -> Self {
        Self {
            used: Vec::new(),
            scope_marks: Vec::new(),
        }
    }

    /// 指定したレジスタを使用中として記録する
    /// Some(..)の場合使われていないレジスタを返す
    pub fn mark_used(&mut self, reg: usize) -> bool {
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

    /// 記録している使用中のレジスタの情報を全て消去する
    /// (関数一つ分のアセンブリ言語の生成が終わった際などに使う)
    pub fn clear(&mut self) {
        self.used.clear();
        self.scope_marks.clear();
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