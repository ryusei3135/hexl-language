//! 命令を`ir_tree`へ積む、`insert`と`builder`共通の小さなAPI

use super::*;

impl IR {
    /// 命令を`ir_tree`へ積み、その命令のidを返す
    pub(in crate::compiler::ir) fn push_inst(&mut self, inst: inst::Inst) -> usize {
        self.ir_tree.push(inst);
        self.id_counter += 1;
        self.id_counter - 1
    }

    /// `value_idx <kind> n`(`n`は即値)の比較命令を積む
    /// (条件分岐の判定用なので、即値の型は`DD`にしている)
    pub(in crate::compiler::ir) fn push_cmp_with_num(
        &mut self,
        value_idx: usize,
        n: usize,
        kind: inst::ExprKind,
    ) -> usize {
        let num_idx = self.id_counter;
        self.push_inst(inst::Inst::gen_num(
            &n.to_string(),
            &types::Size::DD,
            num_idx,
        ));
        let dst = self.id_counter;
        self.push_inst(
            inst::ExprInst {
                dst,
                ls: value_idx,
                rs: num_idx,
                kind,
            }
            .new(),
        )
    }
}