use super::*;
use crate::compiler::ast::{ConstractTy, IS_MUST};

#[test]
fn return_contract_qualifies_matching_declared_method_contract() {
    let owner_ty = TyNode::Ty("Cmdln".to_string());
    let mut flags = ContractFlags::new();
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

#[test]
fn return_contract_matches_pointer_with_declared_range() {
    let returned_ty = TyNode::Pointer {
        is_const: false,
        ty_name: Box::new(TyNode::Ty("byte".to_string())),
        range: None,
    };
    let declared_ty = TyNode::Pointer {
        is_const: false,
        ty_name: Box::new(TyNode::Ty("byte".to_string())),
        range: Some((0, usize::MAX)),
    };
    let mut flags = ContractFlags::new();
    flags.constract_ret = Some(ConstractTy::new::<{ IS_MUST }>(
        Some("free".to_string()),
        None,
        Box::new(returned_ty),
    ));
    let declared_contract =
        ConstractTy::new::<{ IS_MUST }>(Some("free".to_string()), None, Box::new(declared_ty));

    assert_eq!(
        flags.var_ty_with_return_contract(&declared_contract),
        flags.constract_ret.clone().unwrap()
    );
    assert!(flags.put_var_def(&declared_contract).is_ok());
    assert!(flags.constract_ret.is_none());
}
