use super::*;

#[test]
fn get_param_ty_infers_arithmetic_expression_type() {
    let insts = vec![
        Inst::gen_num("1", &types::Size::DQ, 0),
        Inst::gen_num("2", &types::Size::DQ, 1),
        ExprInst {
            dst: 2,
            ls: 0,
            rs: 1,
            kind: ExprKind::Add,
        }
        .new(),
    ];

    assert_eq!(insts[2].get_param_ty(&insts), Some(types::Size::DQ));
}

#[test]
fn get_param_ty_uses_boolean_type_for_comparison_expression() {
    let insts = vec![
        Inst::gen_num("1", &types::Size::F64, 0),
        Inst::gen_num("2", &types::Size::F64, 1),
        ExprInst {
            dst: 2,
            ls: 0,
            rs: 1,
            kind: ExprKind::SetLessThen,
        }
        .new(),
    ];

    assert_eq!(insts[2].get_param_ty(&insts), Some(types::Size::DB));
}
