use super::*;
use crate::compiler::ast::TyNode;
use crate::compiler::ir::IR;

mod constract;
mod mem;
mod tracking;
mod var_ty;

// 範囲外のときの分岐IRを生成する`ir/insert/array_guard.rs`が使う
// (`builder`が`check_mem_access`の結果を`insert`へ渡す)
pub(in crate::compiler::ir) use mem::access::ArrIdxGuard;

use crate::compiler::models::Body;

#[derive(Debug)]
pub(in crate::compiler::ir) struct ConstractFlags {
    /// 変数を定義
    /// これは戻り値が契約のフラグを立てたときにtrueならOk
    def_var: Option<TyNode>,
    /// 戻り値が契約
    constract_ret: Option<TyNode>,
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
    pub(in crate::compiler::ir) fn put_var_def(&mut self, var_ty: &TyNode) {
        if self.constract_ret.is_none() {
            self.def_var = Some(var_ty.clone());
        } else {
            if self.constract_ret.as_ref().unwrap() != var_ty {
                panic!("{:?} {:?}", var_ty, self.constract_ret);
            }
            self.reset();
        }
    }

    /// 初期化式が持つ契約を、型が一致する変数宣言へ引き継ぐ
    pub(in crate::compiler::ir) fn var_ty_with_return_contract(&self, var_ty: &TyNode) -> TyNode {
        let Some(contract_ty) = self.constract_ret.as_ref() else {
            return var_ty.clone();
        };
        let (inner_ty, returned_is_must, returned_contract) = match contract_ty {
            TyNode::ConstractMust(contract) => (contract.unwrap_ty(), true, contract),
            TyNode::ConstractOf(contract) => (contract.unwrap_ty(), false, contract),
            _ => return var_ty.clone(),
        };

        if &inner_ty == var_ty {
            return contract_ty.clone();
        }
        let (declared_is_must, declared_contract) = match var_ty {
            TyNode::ConstractMust(contract) => (true, contract),
            TyNode::ConstractOf(contract) => (false, contract),
            _ => return var_ty.clone(),
        };
        let Some(declared_name) = declared_contract.must_name() else {
            return var_ty.clone();
        };
        let declared_name = if declared_name.contains("::") {
            declared_name.clone()
        } else {
            match &inner_ty {
                TyNode::Ty(owner) | TyNode::SelfTy(owner) => {
                    format!("{owner}::{declared_name}")
                }
                _ => declared_name.clone(),
            }
        };
        if declared_is_must == returned_is_must
            && declared_contract.unwrap_ty() == inner_ty
            && returned_contract
                .must_name()
                .is_some_and(|name| name == &declared_name)
        {
            contract_ty.clone()
        } else {
            var_ty.clone()
        }
    }

    #[inline(always)]
    pub(in crate::compiler::ir) fn constract_fn(&mut self, fn_ret_ty: Option<&TyNode>) {
        if self.def_var.is_none() {
            if self.constract_ret.is_none() {
                self.constract_ret = fn_ret_ty.map(|v| v.clone());
                return ();
            }
        }
        // 型をちぇく
        if &self.def_var != &fn_ret_ty.map(|v| v.clone()) {
            panic!("型が違う {:?} {:?}", self.def_var, fn_ret_ty);
        }
        self.def_var = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::ast::{ConstractTy, IS_MUST};

    #[test]
    fn return_contract_qualifies_matching_declared_method_contract() {
        let owner_ty = TyNode::Ty("Cmdln".to_string());
        let mut flags = ConstractFlags::new();
        flags.constract_ret = Some(ConstractTy::new::<{ IS_MUST }>(
            Some("Cmdln::del".to_string()),
            None,
            Box::new(owner_ty.clone()),
        ));
        let declared_ty =
            ConstractTy::new::<{ IS_MUST }>(Some("del".to_string()), None, Box::new(owner_ty));

        assert_eq!(
            flags.var_ty_with_return_contract(&declared_ty),
            flags.constract_ret.clone().unwrap()
        );
    }
}
