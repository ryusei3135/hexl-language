//! スコープ(`StartScope`/`EndScope`)と、反復処理の終了時のレジスタ解放

use super::*;

impl AsmEmitter {
    /// `Inst::StartScope`の処理
    /// これ以降に登録されたレジスタを記録し始める
    pub(in crate::compiler::asm_gen) fn start_scope_asm(&mut self) {
        self.used_reg.start_scope();
        self.fn_state
            .scope_outer_vars
            .push(self.fn_state.var_hash_map.keys().cloned().collect());
    }

    /// `Inst::EndScope`の処理
    /// `StartScope`以降に登録されたレジスタを全て解放する。
    ///
    /// 解放したレジスタのうち、空いている最小の番号を`self.reg_idx`に
    /// 設定するので、スコープ内で使ったレジスタを次の式が再利用できる。
    ///
    /// ただし、スコープの外で宣言された変数が(スコープ内での再代入などで)
    /// レジスタを使うようになった場合、そのレジスタはスコープが終わっても
    /// 生きているので、解放せずに残す
    pub(in crate::compiler::asm_gen) fn end_scope_asm(&mut self) {
        let released = self.used_reg.end_scope();
        let outer_vars = self
            .fn_state
            .scope_outer_vars
            .pop()
            .expect("StartScopeに対応しないEndScopeです");
        // スコープの中で定義した変数は、もう使われない
        let dead: Vec<String> = self
            .fn_state
            .var_hash_map
            .keys()
            .filter(|name| !outer_vars.contains(name))
            .cloned()
            .collect();
        self.fn_state.dead_vars.extend(dead);
        // レジスタを持たない変数(静的領域・配列の実体など)は対象外
        let live_regs: Vec<RegisterMode> = outer_vars
            .iter()
            .filter(|name| {
                // 最後の使用を終えた変数(`dead_vars`)のレジスタは戻さない
                !self.fn_state.dead_vars.contains(name.as_str())
                    && self.find_var_info(name).is_some_and(|var| var.holds_reg())
            })
            .filter_map(|name| self.var_reg(name))
            .collect();
        self.used_reg.restore(&live_regs);
        // 解放されたレジスタの退避は、復元せずに捨てる
        self.drop_dead_spills();

        // 次の式の結果を置くレジスタ(`reg_idx`)を、解放したレジスタに戻す。
        // 外側の変数が使っているレジスタ(上で登録し直したもの)は
        // 除き、`reg_idx`より小さい番号に限る(番号を進めることはしない)
        if let Some(RegisterMode::Normal(reg)) = self.used_reg.lowest_free(&released) {
            if *reg < self.fn_state.reg_idx {
                self.fn_state.reg_idx = *reg;
            }
        }
    }

    /// 条件付きの反復処理(`loop <条件>`)の終了位置(`Block(end)`)で、
    /// 条件の評価に使ったレジスタを解放する
    ///
    /// 反復処理のIRは次の形をしている。
    ///
    /// ```text
    /// Block(start)
    /// ExpectJmp(cond)
    ///   ..条件の式..        <- ここの式の結果は、条件ジャンプで使い終わる
    /// Jmp(end)
    /// Block(cond)
    ///   StartScope .. EndScope
    /// Jmp(start)
    /// Block(end)            <- `block_idx`
    /// ```
    ///
    /// 条件の式の結果のレジスタは、`StartScope`より前に確保されて誰にも
    /// 消費されないため、`EndScope`でも解放されず、反復処理が終わった後も
    /// 使用中のまま残っていた。反復処理が終わった時点で、条件の式の
    /// 一時的な結果を`release_expr_temp`で解放する(変数が持っている
    /// レジスタは解放しない)。
    ///
    /// `block_idx`が条件付きの反復処理の終了位置でなければ何もしない。
    pub(in crate::compiler::asm_gen) fn release_loop_cond_regs(&mut self, block_idx: usize) {
        let Some((first, last)) = self.loop_cond_range(block_idx) else {
            return;
        };
        let mut released: Vec<RegisterMode> = Vec::new();
        for idx in first..last {
            let reg = self
                .fn_state
                .last_inst_idx
                .iter()
                .find(|(id, _)| *id == idx)
                .map(|(_, reg)| *reg);
            let was_used = reg.is_some_and(|reg| self.used_reg.is_used(&reg));
            self.release_expr_temp(idx);
            if let Some(reg) = reg {
                if was_used && !self.used_reg.is_used(&reg) {
                    released.push(reg);
                }
            }
        }
        // 次の式の結果を置くレジスタ(`reg_idx`)を、解放したレジスタに戻す
        // (`end_scope_asm`と同じ。番号を進めることはしない)
        if let Some(RegisterMode::Normal(reg)) = self.used_reg.lowest_free(&released) {
            if *reg < self.fn_state.reg_idx {
                self.fn_state.reg_idx = *reg;
            }
        }
    }

    /// `block_idx`が条件付きの反復処理の終了位置なら、条件の式のノードの
    /// 範囲`[first, last)`を返す
    fn loop_cond_range(&self, block_idx: usize) -> Option<(usize, usize)> {
        let inst::Inst::Block(end) = self.fn_state.curr_inst.get(block_idx)? else {
            return None;
        };
        // 直前の`Jmp(start)`は、反復処理の先頭へ戻る後ろ向きのジャンプ
        let inst::Inst::Jmp(start) = self.fn_state.curr_inst.get(block_idx.checked_sub(1)?)? else {
            return None;
        };
        if start == end {
            return None;
        }
        let start_idx = self.fn_state.curr_inst[..block_idx]
            .iter()
            .position(|node| matches!(node, inst::Inst::Block(name) if name == start))?;
        // 条件がある反復処理は、先頭の直後が`ExpectJmp`
        if !matches!(
            self.fn_state.curr_inst.get(start_idx + 1),
            Some(inst::Inst::ExpectJmp(_))
        ) {
            return None;
        }
        // 条件が偽のときの`Jmp(end)`(本体の`break`より前にある最初のもの)
        let cond_jmp_idx = (start_idx + 1..block_idx).find(
            |idx| matches!(&self.fn_state.curr_inst[*idx], inst::Inst::Jmp(name) if name == end),
        )?;
        Some((start_idx + 2, cond_jmp_idx))
    }
}
