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
use crate::compiler::ast::Expr;

impl IR {
    /// 添字が可変な配列アクセスのIRを、範囲外のときの分岐つきで生成する
    ///
    /// 分岐の形は`insert_guarded_store`を参照(添字を一時変数に写して、
    /// 範囲外なら`else`の値に直し、`InsertArr`は1回だけ積む)。
    /// IR上では`ExpectJmp(L)`+条件式が「条件が真なら`L`へ」を表す。
    ///
    /// ## Returns
    /// 最後に積んだ`InsertArr`のid
    pub(in crate::compiler::ir) fn insert_arr_guard<M: ResultMod>(
        &mut self,
        guard: checker::ArrIdxGuard,
        dst: &Expr,
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
    ///
    /// `InsertArr`は単独では1行も出力しないオペランドのノードなので、
    /// 分岐の両側に積むのではなく、添字の方を一時変数に入れて直す:
    /// ```text
    ///   tmp = index                      // Mov(名前付きの一時変数)
    ///   [ExpectJmp(else); tmp < lo]      // lo > 0 のときだけ
    ///   ExpectJmp(end);   tmp <= hi - 1  // 範囲内なら直さず end へ
    /// else:
    ///   tmp = else_idx                   // AssignVar
    /// end:
    ///   InsertArr(arr, dst, tmp)         // 1回だけ。このidを返す
    /// ```
    ///
    /// ## Returns
    /// `store`が最後に積んだ命令(`InsertArr`)のid
    pub(in crate::compiler::ir) fn insert_guarded_store<M: ResultMod>(
        &mut self,
        guard: checker::ArrIdxGuard,
        dst: &Expr,
        expect_byte: &types::Size,
        store: impl Fn(&mut Self, usize, usize),
    ) -> Result<usize, err::ErrKind> {
        let else_label = self.next_pattern_label();
        let end_label = self.next_pattern_label();

        let dst_idx = self.gen_expr_ir::<M>(&dst, Some(expect_byte))?;
        let index_ty = self.index_expect_ty(&guard.index, expect_byte);
        let src_idx = self.gen_expr_ir::<M>(&guard.index, Some(&index_ty))?;

        // 添字を一時変数へ写す(`movl %edi, %ecx`)
        let tmp_name = format!("__arr_idx{}", self.id_counter);
        let tmp_idx = self.id_counter;
        self.push_inst(inst::Inst::Mov {
            name: Some(tmp_name.clone()),
            size: index_ty.clone(),
            dst: tmp_idx,
            src: src_idx,
        });
        // 使用回数0回の変数は生成されないので、比較と`InsertArr`の2回ぶん数える
        self.var_counter.use_var(&tmp_name);
        self.var_counter.use_var(&tmp_name);

        self.insert_range_check_jmp(tmp_idx, guard.lo, guard.hi, else_label, end_label);

        // 範囲外: 添字を`else`の値にする
        crate::push_jmp_code!(self, Block, &else_label);
        let else_ty = self.index_expect_ty(&guard.else_idx, expect_byte);
        let else_idx = M::handle::<usize>(self.gen_expr_ir::<M>(&guard.else_idx, Some(&else_ty)))?;
        self.push_inst(inst::Inst::AssignVar {
            name: tmp_name,
            dst: tmp_idx,
            value: else_idx,
        });

        crate::push_jmp_code!(self, Block, &end_label);
        store(self, dst_idx, tmp_idx);
        Ok(self.id_counter - 1)
    }

    /// 範囲内なら`end_label`へ、範囲外ならそのまま(直後の`else`)へ進む判定を差し込む
    ///
    /// `lo == 0`(配列)なら、符号なしの添字は`idx <= hi - 1`の1回で済む
    /// (負の添字は符号なしでは大きな値になるので、これで弾ける)。
    /// `lo > 0`(範囲付きポインタ)は、先に`idx < lo`を判定して`else`へ飛ばす。
    /// `hi > lo`は`check_ref_array`で確認済みなので、`hi - 1`はunderflowしない
    fn insert_range_check_jmp(
        &mut self,
        index_idx: usize,
        lo: usize,
        hi: usize,
        else_label: usize,
        end_label: usize,
    ) {
        if lo > 0 {
            crate::push_jmp_code!(self, ExpectJmp, &else_label);
            self.push_cmp_with_num(index_idx, lo, inst::ExprKind::LessThen);
        }
        crate::push_jmp_code!(self, ExpectJmp, &end_label);
        self.push_cmp_with_num(index_idx, hi - 1, inst::ExprKind::LessThenEq);
    }
}
