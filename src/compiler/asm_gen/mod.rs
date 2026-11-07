//! このモジュールは、IRで生成したものを
//! 渡されたフォーマットのアセンブリ言語に変換するApiを提供する

/// self.format_lineで構造体のポインタを渡すところがある
mod access;
use access::FRAME_BASE_REG;
mod asm_emitter;
mod asm_fmt;
mod call_func;
mod emit_fn_name;
mod fn_state;
mod inline_asm;
mod last_use;
mod lazy_spill;
mod reg_mnger;
mod sign;

use crate::compiler::err::{self, err_pool};
use crate::compiler::ir::{def_tree, inst, types::Size};
use crate::compiler::models::AsmFmtData;
use fn_state::FnState;
use sign::SizeSign;
use std::collections::HashMap;
use std::mem;

use std::cell::RefCell;
use std::rc::Rc;

pub type SelfPtrInfo<'a> = Option<&'a Size>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Ord, PartialOrd)]
pub(in crate::compiler::asm_gen) enum RegisterMode {
    Normal(usize),
    Float(usize),
}

impl RegisterMode {
    pub fn index(&self) -> usize {
        match self {
            Self::Normal(idx) | Self::Float(idx) => *idx,
        }
    }

    pub fn unwrap_normal(&self) -> usize {
        if let Self::Normal(idx) = self {
            *idx
        } else {
            panic!();
        }
    }
}

/// 演算結果を格納するレジスタの「適切なサイズ」が、テンプレートを
/// 組み立てる時点ではまだ決まらない演算子の一覧。
///
/// `*`(Mul)/`/`(Div)/`%`(Surplus)は`idiv`/`imul`を使うため、結果を
/// 格納するレジスタのサイズが、オペランドがメモリ上の値かどうかを
/// 見て初めて確定する(`format_expr_inst`内の`check_node_is_memory_value`
/// による判定より後)。そのため、テンプレートを組み立てる時点では
/// レジスタの「番号」だけを埋め込んでおき([`AsmEmitter::replace_insert_fmt_reg`]
/// を参照)、サイズが確定してから改めて実際のレジスタ名へ展開し直す
/// 必要がある。
#[allow(dead_code)]
pub(in crate::compiler::asm_gen) const DEFERRED_REG_FMT_OPS: &[inst::ExprKind; 3] = &[
    inst::ExprKind::Mul,
    inst::ExprKind::Div,
    inst::ExprKind::Surplus,
];

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum OptMode {
    O0,
    O1,
}

/// 式の結果や変数を置くレジスタとして、自動では割り当てないレジスタ。
///
/// - `0`: `%rax` 戻り値・`idiv`の商・システムコール番号
/// - `2`: `%rdx` `idiv`の余り・`cqto`/`cltd`の結果・第3引数
///
/// `/`と`%`は、この2つのレジスタを必ず書き換える。一般の式の結果を
/// ここに置いてしまうと、割り算のたびに壊れてしまう(引数のレジスタが
/// 破壊されるバグの原因の一つだった)ので、割り当て対象から外す。
pub(in crate::compiler::asm_gen) const RESERVED_REGS: [usize; 2] = [0, 2];

pub struct AsmEmitter {
    pub(in crate::compiler::asm_gen) asm_text: String,
    pub(in crate::compiler::asm_gen) data_sec_text: String,
    pub(in crate::compiler::asm_gen) data_idx: usize,

    pub(in crate::compiler::asm_gen) asm_fmt: asm_fmt::mng_fmt::MngAsmFmt,

    pub(in crate::compiler::asm_gen) os: String,

    /// 関数ごとの状態(詳細は`fn_state.rs`)
    pub(in crate::compiler::asm_gen) fn_state: FnState,
    // 現在使用中のレジスタを管理する
    pub(in crate::compiler::asm_gen) used_reg: reg_mnger::UsedRegManager,

    /// まだ生成していない`_start`以外の関数(ラベル -> 関数の定義)。
    /// `emit_call_func`が呼び出しを生成するときに取り出して、
    /// `build_fn_process`で生成する(呼ばれない関数は残ったまま出力されない)
    pub(in crate::compiler::asm_gen) pending_funcs: HashMap<String, (String, def_tree::FnDefInfo)>,
    /// `emit_call_func`が生成した、呼ばれた側の関数のアセンブリ。
    /// 呼んだ側の関数の途中に挟まらないよう、別に溜めて最後に結合する
    pub(in crate::compiler::asm_gen) fn_asm_out: String,
    /// 出力するアセンブリ言語のフォーマットの名前
    /// (呼ばれた関数を`build_fn_process`で生成するときに使う)
    pub(in crate::compiler::asm_gen) asm_fmt_name: Option<String>,

    callee_saved: Rc<Vec<usize>>,
    caller_saved: Rc<Vec<usize>>,
    float_callee_saved: Rc<Vec<usize>>,
    float_caller_saved: Rc<Vec<usize>>,

    err_pool: Rc<RefCell<err_pool::ErrPool>>,

    optimization_mode: OptMode,
}

pub trait ResultRet {
    fn handle(
        err_pool: &Rc<RefCell<err_pool::ErrPool>>,
        val: Result<(), err::ErrKind>,
    ) -> Result<(), err::ErrKind>;
}

pub struct StackErr {}

impl ResultRet for StackErr {
    fn handle(
        err_pool: &Rc<RefCell<err_pool::ErrPool>>,
        val: Result<(), err::ErrKind>,
    ) -> Result<(), err::ErrKind> {
        if let Err(e) = val.clone() {
            (*err_pool.borrow_mut()).push_err_node(e);
        }
        Ok(())
    }
}

pub struct ReturnErr {}

impl ResultRet for ReturnErr {
    fn handle(
        err_pool: &Rc<RefCell<err_pool::ErrPool>>,
        val: Result<(), err::ErrKind>,
    ) -> Result<(), err::ErrKind> {
        panic!("{:?}", val);
        Ok(())
    }
}
