//! `[arr idx else 0]`(添字が可変な配列アクセス)の範囲外ガードを差し込む
//!
//! `checker/access_mem.rs`の`check_ref_array`が、添字が可変で`else`が
//! 正しく指定されていた場合に`ArrIdxGuard`を返す。ここではそれを受け取り、
//! 次の形になるようにIRへ条件分岐を差し込む:
//!
//! ```text
//! if lo <= idx < hi { arr[idx] = dst } else { arr[else_idx] = dst }
//! ```
//!
//! 配列なら`lo = 0`、`hi = 配列の長さ`なので、
//! `if idx < arr_len { arr[idx] } else { arr[else_idx] }`になる。

use super::*;

impl IR {
    /// 添字が可変な配列アクセスのIRを、範囲外のときの分岐つきで生成する
    ///
    /// IR上では`ExpectJmp(L)`+条件式が「条件が真なら`L`へ」を表すので、
    /// 範囲外になる条件(`idx < lo`と`idx > hi - 1`)を順に判定して
    /// どちらかが真なら`else`のブロックへ飛ばす。
    /// ```text
    ///   (dst, idxを評価)
    ///   ExpectJmp(else); idx < lo
    ///   ExpectJmp(else); idx > hi - 1
    ///   InsertArr(arr, dst, idx)        // 範囲内
    ///   Jmp(end)
    /// else:
    ///   InsertArr(arr, dst, else_idx)   // 範囲外
    /// end:
    /// ```
    /// `dst`と`idx`は副作用が二重に起きないよう、分岐の前に一度だけ評価する。
    ///
    /// ## Returns
    /// 最後に積んだ命令(`end`のブロック)のid
    /// 添字が可変な配列アクセスのIRを、範囲外のときの分岐つきで生成する
    pub(in crate::compiler::ir) fn insert_arr_guard<M: ResultMod>(
        &mut self,
        guard: checker::ArrIdxGuard,
        dst: node::Expr,
        expect_byte: &types::Size,
    ) -> Result<usize, err::ErrKind> {
        let name = guard.name.clone();
        self.insert_guarded_store::<M>(guard, dst, expect_byte, move |ir, dst, index| {
            ir.push_inst(inst::Inst::InsertArr {
                name: name.clone(),
                dst,
                index,
            });
        })
    }

    /// 範囲外のときの分岐つきで、`store(ir, dst_idx, index_idx)`が出す格納命令を生成する
    /// (配列/ポインタのメンバーで骨組みを共有する)
    pub(in crate::compiler::ir) fn insert_guarded_store<M: ResultMod>(
        &mut self,
        guard: checker::ArrIdxGuard,
        dst: node::Expr,
        expect_byte: &types::Size,
        store: impl Fn(&mut Self, usize, usize),
    ) -> Result<usize, err::ErrKind> {
        let else_label = self.next_pattern_label();
        let end_label = self.next_pattern_label();

        let dst_idx = self.gen_expr_ir::<M>(dst, Some(expect_byte))?;
        let index_ty = self.index_expect_ty(&guard.index, expect_byte);
        let index_idx = self.gen_expr_ir::<M>(guard.index.clone(), Some(&index_ty))?;

        self.insert_out_of_range_jmp(index_idx, guard.lo, guard.hi, else_label);

        // 範囲内
        store(self, dst_idx, index_idx);
        crate::push_jmp_code!(self, Jmp, &end_label);

        // 範囲外
        crate::push_jmp_code!(self, Block, &else_label);
        let else_ty = self.index_expect_ty(&guard.else_idx, expect_byte);
        let else_idx =
            M::handle::<usize>(self.gen_expr_ir::<M>(guard.else_idx.clone(), Some(&else_ty)))?;
        store(self, dst_idx, else_idx);

        crate::push_jmp_code!(self, Block, &end_label);
        Ok(self.id_counter - 1)
    }

    /// `index_idx`が`lo..hi`の範囲外なら`else_label`へ飛ぶ条件式を差し込む
    ///
    /// `idx < lo`と`idx > hi - 1`(= `idx >= hi`)の2つのどちらかが真なら範囲外。
    /// `hi > lo`は`check_ref_array`で確認済みなので、`hi - 1`はunderflowしない
    fn insert_out_of_range_jmp(
        &mut self,
        index_idx: usize,
        lo: usize,
        hi: usize,
        else_label: usize,
    ) {
        crate::push_jmp_code!(self, ExpectJmp, &else_label);
        self.push_cmp_with_num(index_idx, lo, inst::ExprKind::LessThen);

        crate::push_jmp_code!(self, ExpectJmp, &else_label);
        self.push_cmp_with_num(index_idx, hi - 1, inst::ExprKind::GreaterThen);
    }
}
