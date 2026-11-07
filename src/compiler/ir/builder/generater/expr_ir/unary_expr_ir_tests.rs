use super::*;

fn has_kind(body: &[inst::Inst], expected: inst::ExprKind) -> bool {
    body.iter().any(|inst| match inst {
        inst::Inst::Expr(e) => e.kind == expected,
        _ => false,
    })
}

#[test]
fn negative_sign_expands_to_sub_from_zero() {
    let body = build_func_body("_start(): int { a: int = 10 b: int = -a ret b }");
    assert!(has_kind(&body, inst::ExprKind::Sub), "{:?}", body);
}

#[test]
fn positive_sign_generates_no_extra_inst() {
    let plain = build_func_body("_start(): int { a: int = 10 b: int = a ret b }");
    let signed = build_func_body("_start(): int { a: int = 10 b: int = +a ret b }");
    assert_eq!(plain.len(), signed.len());
}

#[test]
fn bit_inversion_expands_to_xor() {
    let body = build_func_body("_start(): int { a: int = 10 b: int = ~a ret b }");
    assert!(has_kind(&body, inst::ExprKind::XorCalcul), "{:?}", body);
}

#[test]
fn bit_inversion_of_unsigned_uses_type_max_mask() {
    // 符号なしに`-1`は入らないので、型の最大値をマスクにする
    let body = build_func_body("_start(): uint { a: uint = 10 b: uint = ~a ret b }");
    assert!(has_kind(&body, inst::ExprKind::XorCalcul), "{:?}", body);
}

#[test]
fn logical_not_expands_to_equal_zero() {
    let body = build_func_body("_start(): int { a: int = 10 b: int = !a ret b }");
    assert!(has_kind(&body, inst::ExprKind::Equal), "{:?}", body);
}

#[test]
fn unary_minus_literal_in_comparison_is_typed_like_other_operand() {
    // `a < -1`の`-1`は、`a`の型(定義元の型)になる
    let body = build_func_body("_start(): int { a: int = 10 cond a < -1 { b: int = 1 } }");
    assert!(has_kind(&body, inst::ExprKind::LessThen), "{:?}", body);
}
