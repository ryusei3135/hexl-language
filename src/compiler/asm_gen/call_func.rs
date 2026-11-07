mod args;
mod assign_var;
mod caller_save;
mod expr_asm;
mod fn_build;
mod mem_ir;
mod node_asm;

use super::*;
use crate::compiler::asm_gen::{self, emit_fn_name::*};
use crate::compiler::ir::{self, types};
use crate::compiler::{ast, err};

/// 関数の入口で`callee_saved`のレジスタを退避する位置の目印
/// (本体の生成が終わってから、実際に使ったレジスタの`push`に置き換える)
const CALLEE_SAVE_MARK: &str = "{callee_save}";
/// `leave`/`ret`の直前で、退避したレジスタを戻す位置の目印
const CALLEE_RESTORE_MARK: &str = "{callee_restore}";

/// スタックで渡された最初の引数の、`%rbp`からのオフセット
/// (`push %rbp`で保存した`%rbp`(8byte)と戻りアドレス(8byte)の上)
const STACK_PARAM_BASE: usize = 16;

/// 1つの関数のアセンブリを生成している間、各ノードの生成で共通して
/// 参照する情報(`build_fn_process`で作り、`gen_node_asm`へ渡す)
struct FnGenCtx<'a> {
    /// 生成中の関数の名前(`_start`かどうかの判定に使う)
    fn_name: &'a str,
    /// 関数の戻り値の型(戻り値が無い/構造体を返す関数は`None`)
    fn_ret_ty: SelfPtrInfo<'a>,
    /// 先頭の引数が`self`か
    this_is_self: bool,
    /// 戻り値として返される構造体の`Inst::Struct`のid
    returned_struct_idx: Option<usize>,
    /// 出力するアセンブリ言語のフォーマットの名前
    asm_fmt_name: &'a Option<&'a str>,
}

impl AsmEmitter {
    /// 関数のラベル名を作る。ジェネリクス関数は型引数を名前に含める
    pub(super) fn fn_label_of<T: std::fmt::Debug>(name: &str, temp_ty: &[T]) -> String {
        if temp_ty.is_empty() {
            emit_fn_name_id(name)
        } else {
            let generic_args = temp_ty
                .iter()
                .map(|ty| format!("{:?}", ty))
                .collect::<Vec<_>>();
            emit_generic_fn_name_id(name, &generic_args)
        }
    }

    /// 関数を呼び出す情報がある物を受け取りアセンブリ言語を生成する
    ///
    /// `gen/asm_emitter.rs`の`extract_operand_text`から、関数呼び出しの
    /// 結果を値として使う(戻り値を任意のレジスタへ代入する)際にも
    /// 使われるため`pub(super)`にしている
    ///
    /// 流れ: callerレジスタの退避 → 引数の設定 → `call` → 退避したレジスタの復元
    pub(super) fn emit_call_func(
        &mut self,
        meta_data: &inst::CallFuncMetaData,
        expand_struct_return: bool,
    ) -> Result<String, err::ErrKind> {
        // 呼ぶ関数がまだ生成されていなければ、ここで`build_fn_process`で生成する
        // (呼んだ側の生成中の状態は退避・復元するので影響しない)
        let fn_label = Self::fn_label_of(&meta_data.name, &meta_data.temp_ty);
        if let Some(mut callee) = self.pending_funcs.remove(&fn_label) {
            self.emit_callee_fn(&mut callee);
        }

        // 生成するアセンブリコード
        let mut call_func = String::new();

        // 関数を呼ぶためレジスタを退避
        let protected_regs: Vec<RegisterMode> = self
            .used_reg
            .used_regs()
            .into_iter()
            .filter(|reg| *reg != RegisterMode::Normal(0))
            .collect();
        // 変数のレジスタは領域へ`mov`で退避し、その変数が参照されるまで復元しない
        // (`lazy_spill.rs`)。退避中のレジスタは中身が壊れているので保存しない。
        // それ以外(式の一時的な結果など)は、従来どおり`push`/`pop`
        let mut lazy_regs: Vec<RegisterMode> = Vec::with_capacity(protected_regs.len());
        let mut eager_regs: Vec<RegisterMode> = Vec::with_capacity(protected_regs.len());
        for reg in protected_regs {
            if self.fn_state.spilled_regs.contains_key(&reg) || self.is_callee_saved(reg) {
                continue;
            }
            if self.can_spill_lazily(reg) {
                lazy_regs.push(reg);
            } else {
                eager_regs.push(reg);
            }
        }
        call_func.push_str(&self.spill_store_regs(&lazy_regs)?);
        // `push`と`pop`は同じ`regs`から同時に作り、対応が崩れないようにする。
        // 退避の合計が16byteの奇数倍なら、`call`の直前に`%rsp`が16byte境界に
        // なるよう、8byteの余白を挟む(System V AMD64 ABI)
        let (caller_save_asm, caller_restore_asm) = self.caller_save_pair(&eager_regs)?;
        call_func.push_str(&caller_save_asm);

        // 関数の引数に使ったレジスタを解放するためにスコープを作ったことにする
        self.used_reg.start_scope();
        let (arg_lines, stack_args) = self.build_arg_lines(meta_data, expand_struct_return)?;
        // 引数レジスタに収まらない引数は、最後の引数から順にスタックへ`push`する。
        // 引数レジスタへ値を書く前に積む(積む値を読むレジスタが、先に上書きされないように)。
        // 積んだ量が16byteの奇数倍なら、`call`の時点で`%rsp`が16byte境界になるよう
        // 8byteの余白を先に確保する(退避は`caller_save_pair`が16byteの倍数にしている)
        let stack_arg_bytes = stack_args.len() * 8;
        let pad = if stack_args.len() % 2 == 1 { 8 } else { 0 };
        if pad != 0 {
            call_func.push_str(&self.asm_fmt.get_stack_alloc_8());
        }
        for push_asm in stack_args.iter().rev() {
            call_func.push_str(push_asm);
        }
        call_func.push_str(&self.order_arg_lines(arg_lines)?);

        call_func.push_str(&self.asm_fmt.get_call_func_fmt(&fn_label));
        // 引数で使ったレジスタを解放
        self.used_reg.end_scope();
        // 呼び出し側が、スタックで渡した引数(と余白)を解放する
        if stack_arg_bytes + pad != 0 {
            call_func.push_str(&self.asm_fmt.get_stack_free_bytes(stack_arg_bytes + pad));
        }

        // 退避した順の逆順に復元する(余白があれば先に解放する)
        call_func.push_str(&caller_restore_asm);
        call_func.push_str(&self.after_call_spills(&lazy_regs)?);
        Ok(call_func)
    }

    /// 呼ばれた関数1つぶんを`build_fn_process`で生成し、`fn_asm_out`へ溜める
    ///
    /// 呼んだ側の関数の生成の途中で呼ばれることがあるため、関数ごとの状態
    /// (`FnState`と`used_reg`)と`asm_text`を退避し、
    /// 生成が終わったら元に戻す。呼ばれた関数の中で、さらに別の関数を
    /// 呼ぶ場合も同じ処理で再帰的に生成される
    pub(super) fn emit_callee_fn(&mut self, callee: &mut (String, def_tree::FnDefInfo)) {
        // 関数ごとの状態は`fn_state`へ集約しているので、丸ごと退避・復元する
        // (`used_reg`だけは`Default`へ戻せないので`clone`で退避する)
        let asm_text = mem::take(&mut self.asm_text);
        let fn_state = mem::take(&mut self.fn_state);
        let used_reg = self.used_reg.clone();

        let asm_fmt_name = self.asm_fmt_name.clone();
        self.build_fn_process(callee, asm_fmt_name.as_deref());
        let callee_asm = mem::replace(&mut self.asm_text, asm_text);
        self.fn_asm_out.push_str(&callee_asm);

        self.fn_state = fn_state;
        self.used_reg = used_reg;
    }
}
