use crate::node;
use std::collections::HashMap;
use std::mem;

pub mod builder;
/// 変数や関数、構造体などの、定義を一時的に
/// 保存する構造体を提供するモジュール
pub mod def_tree;
pub mod inst;
mod param;

mod checker;
pub mod types;

use crate::err;


struct ConstractFlags {
    /// 変数を定義
    /// これは戻り値が契約のフラグを立てたときにtrueならOk
    def_var: bool,
    /// 戻り値が契約
    constract_ret: bool,
}

impl ConstractFlags {
    pub const fn new() -> Self {
        Self {
            def_var: false,
            constract_ret: false,
        }
    }

    #[inline(always)]
    pub fn put_var_def(&mut self) {
        self.def_var = true
    }

    #[inline(always)]
    pub fn constract_fn(&mut self) {
        if self.def_var == false {
            panic!();
        }
        self.def_var = false;
    }
}

pub struct IR {
    pub var_tree: def_tree::VarTree,
    pub extern_funcs: Vec<inst::Inst>,
    id_counter: usize,
    func_ret_ty: Option<node::TyNode>,
    ir_tree: Vec<inst::Inst>,
    pattern_labels: usize,
    loop_labels: Vec<(usize, usize)>,
    scope_states: Vec<def_tree::VarTree>,

    this_is_self: bool,
    /// 式や文を生成する際に、一番最初のノードの場合のみtrue
    /// 関数を呼ぶノードが変数に戻り値を代入市内債などに使う
    expr_counter: usize,
    // 関数の情報
    pub func_tree: def_tree::FuncTree,
    // 外部の関数の情報
    extern_func_tree: Vec<def_tree::FnDefMetaData>,
    // 自身が公開する関数の配列:
    pub public_func_tree: Vec<String>,
    define_meta_data: Vec<def_tree::FnDefMetaData>,
    // 定義済みの構造体の情報
    pub struct_tree: def_tree::StructTree,
    // 定義済みの列挙型の情報
    pub enum_tree: HashMap<String, node::EnumDefine>,
    stk_counter: usize,
    pub(in crate::ir) current_span: err::Span,

    pub(in crate::ir) constract_flag: ConstractFlags,
}

pub const IS_ASSIGN_EXPR: bool = true;
pub const IS_NOT_ASSIGN_EXPR: bool = false;
