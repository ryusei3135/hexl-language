use super::*;
use crate::err::PreprocErrs::NotFoundAsmName;
use crate::ir::IR;

mod var_ty;
mod constract;
mod tracking;

use crate::models::Body;



pub(in crate::ir) 
struct ConstractFlags {
    /// 変数を定義
    /// これは戻り値が契約のフラグを立てたときにtrueならOk
    def_var: Option<node::TyNode>,
    /// 戻り値が契約
    constract_ret: Option<node::TyNode>,
}

/// 一行の式の型をちぇっぅ
/// 行をまたいでのチェックはしない
impl ConstractFlags {
    pub const fn new() -> Self {
        Self {
            def_var: None,
            constract_ret: None,
        }
    }

    pub fn reset(&mut self) {
        self.def_var = None;
        self.constract_ret = None;
    }

    /// var_treeでフラグを立てる
    #[inline(always)]
    pub fn put_var_def(
        &mut self, 
        var_ty: &node::TyNode,
    ) {
        if self.constract_ret.is_none() {
            self.def_var = Some(var_ty.clone());
        } else {
            if !matches!(
                self.constract_ret
                    .as_ref()
                    .unwrap(),
                var_ty
             ) {
                panic!();
            }
            self.reset();
        }
    }

    #[inline(always)]
    pub fn constract_fn(
        &mut self,
        fn_ret_ty: Option<&node::TyNode>,
    ) {
        if self.def_var.is_none() {
            if self.constract_ret.is_none() {
                self.constract_ret = fn_ret_ty
                    .map(|v| v.clone());
                return ();
            }
        }
        // 型をちぇく
        if &self.def_var != &fn_ret_ty
            .map(|v| v.clone()) 
        {
            panic!("型が違う {:?} {:?}", self.def_var, fn_ret_ty);
        }
        self.def_var = None;
    }
}
