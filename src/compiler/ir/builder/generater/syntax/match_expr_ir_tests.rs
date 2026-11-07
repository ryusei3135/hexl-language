use super::*;
use std::collections::HashSet;

#[test]
fn check_match_bool_generates_equal_cmp() {
    // 1. 真偽値(比較式)を与えるパターンは、その式がそのまま
    //    条件分岐の判定に使われる (単純なif/else)
    let body = build_func_body(
        "_start(): int {
            a: int = 10
            cond a == 10 {
                b: int = 1
            } else {
                b: int = 0
            }
        }",
    );
    assert!(
        body.iter().any(|inst| matches!(
            inst,
            inst::Inst::Expr(inst::ExprInst {
                kind: inst::ExprKind::Equal,
                ..
            })
        )),
        "`a == 10` の比較命令(Equal)が生成される必要がある: {:?}",
        body
    );
}

#[test]
fn logical_and_generates_boolean_ir() {
    let body = build_func_body(
        "_start(): bool {
            a: int = 1
            b: int = 2
            ret a && b
        }",
    );

    assert_eq!(
        body.iter()
            .filter(|inst| matches!(
                inst,
                inst::Inst::Expr(inst::ExprInst {
                    kind: inst::ExprKind::SetNotEq,
                    ..
                })
            ))
            .count(),
        2,
        "論理演算のオペランドを真偽値に変換する必要がある: {:?}",
        body
    );
    assert!(
        body.iter().any(|inst| matches!(
            inst,
            inst::Inst::Expr(inst::ExprInst {
                kind: inst::ExprKind::AndCalcul,
                ..
            })
        )),
        "&& のIR(AndCalcul)が生成される必要がある: {:?}",
        body
    );
}

#[test]
fn logical_or_generates_boolean_ir() {
    let body = build_func_body(
        "_start(): bool {
            a: int = 1
            b: int = 2
            ret a < b || b < a
        }",
    );

    assert_eq!(
        body.iter()
            .filter(|inst| matches!(
                inst,
                inst::Inst::Expr(inst::ExprInst {
                    kind: inst::ExprKind::SetLessThen,
                    ..
                })
            ))
            .count(),
        2,
        "|| の比較オペランドはそれぞれ真偽値へ変換される必要がある: {:?}",
        body
    );
    assert!(
        body.iter().any(|inst| matches!(
            inst,
            inst::Inst::Expr(inst::ExprInst {
                kind: inst::ExprKind::OrCalcul,
                ..
            })
        )),
        "|| のIR(OrCalcul)が生成される必要がある: {:?}",
        body
    );
}

#[test]
fn cond_and_expands_to_branches() {
    let body = build_func_body(
        "_start(): int {
            c: int = 100
            cond {
                c >= 97 && c <= 122 => { a: int = 1 }
                else => { a: int = 0 }
            }
            ret 1
        }",
    );

    // 条件の`&&`は、0/1の値を作らず、比較ごとの条件ジャンプへ展開する
    assert!(
        !body.iter().any(|inst| matches!(
            inst,
            inst::Inst::Expr(inst::ExprInst {
                kind: inst::ExprKind::AndCalcul
                    | inst::ExprKind::SetGreaterThenEq
                    | inst::ExprKind::SetLessThenEq,
                ..
            })
        )),
        "条件の&&にAndCalcul/Set*を使ってはいけない: {:?}",
        body
    );
    // 左辺の条件ジャンプと右辺の条件ジャンプで、`ExpectJmp`が2つ以上ある
    assert!(
        body.iter()
            .filter(|inst| matches!(inst, inst::Inst::ExpectJmp(_)))
            .count()
            >= 2,
        "&&の左右それぞれに条件ジャンプが必要: {:?}",
        body
    );
}

#[test]
fn match_arm_block_labels_are_unique() {
    let body = build_func_body(
        "_start(): int {
            cond 1 {
                1 => { a: int = 1 }
                2 => { a: int = 2 }
                else => { a: int = 0 }
            }
            ret 1
        }",
    );

    let labels: Vec<String> = body
        .iter()
        .filter_map(|instruction| match instruction {
            inst::Inst::Block(label) => Some(label.clone()),
            _ => None,
        })
        .collect();
    let unique_labels: HashSet<&String> = labels.iter().collect();

    assert_eq!(
        labels.len(),
        unique_labels.len(),
        "matchのアーム本体ラベルが重複している: {:?}",
        labels
    );
}

const VARIANT_BIND_SRC: &str = "variant V { A B(int) }
    _start() {
        v: V = V::B(10)
        cond B(val) := v {
            x: int = val
        } else {
            x: int = 0
        }
    }";

#[test]
fn check_variant_bind_expands_to_label_compare() {
    // `B(val) := v` は `v.label == tag_of(B)` の比較に展開される
    let body = build_func_body(VARIANT_BIND_SRC);
    let label_pos = body
        .iter()
        .position(|inst| {
            matches!(
                inst,
                inst::Inst::RefStruct { src, pos: 0, size: types::Size::DD } if src == "v"
            )
        })
        .expect("`v.label`(タグの読み出し)が生成される必要がある");
    // `B`は定義順で1番目(`A`が0)なので、タグは1
    assert!(
        matches!(
            &body[label_pos + 1],
            inst::Inst::Num { value, .. } if value == "1"
        ),
        "タグ(1)との比較が続く必要がある: {:?}",
        body
    );
    assert!(
        matches!(
            &body[label_pos + 2],
            inst::Inst::Expr(inst::ExprInst {
                kind: inst::ExprKind::Equal,
                ..
            })
        ),
        "`==`の比較命令が続く必要がある: {:?}",
        body
    );
}

#[test]
fn check_variant_bind_defines_val_from_payload() {
    // 条件が真のときの本体の先頭で、`val = v.value`(タグの後ろのペイロード)が
    // 定義される。それは本体の中の`x: int = val`より前にある
    let body = build_func_body(VARIANT_BIND_SRC);
    let payload_pos = body
        .iter()
        .position(|inst| {
            matches!(
                inst,
                inst::Inst::RefStruct { src, pos, size: types::Size::DD }
                    if src == "v" && *pos == types::VARIANT_TAG_BYTES
            )
        })
        .expect("`v.value`(ペイロードの読み出し)が生成される必要がある");
    let val_pos = body
        .iter()
        .position(|inst| matches!(inst, inst::Inst::Mov { name: Some(n), .. } if n == "val"))
        .expect("`val`の定義(Mov)が生成される必要がある");
    assert!(payload_pos < val_pos, "{:?}", body);
    // `val`の定義は、`ExpectJmp`の先(アームのラベルの後ろ)にある
    let arm_block = body
        .iter()
        .position(|inst| matches!(inst, inst::Inst::Block(_)))
        .expect("アームのラベル(Block)が必要");
    assert!(arm_block < payload_pos, "{:?}", body);
}

#[test]
fn check_variant_ctor_defined_after_use() {
    // バリアント型が使う関数より後ろに定義されていても、
    // `E::b(10)`は関数呼び出しではなく、バリアント型の初期化になる
    let nodes =
        parse_test_source("_start(): int { e: E = E::b(10) ret 1 } variant E { a b(int) c }");
    let mut ir = new_test_ir();
    ir.builder::<crate::compiler::ir::result_mod::PanicErr>(&nodes)
        .unwrap();
    let body = ir.test_only_get_func_body("_start").to_vec();
    assert!(
        body.iter().any(|inst| matches!(
            inst,
            inst::Inst::Variant {
                tag: 1,
                value: Some(_),
                ..
            }
        )),
        "{:?}",
        body
    );
}

#[test]
fn check_variant_bind_with_explicit_ty() {
    // `B(val): int = v` も同じIRになる
    let infer = build_func_body(VARIANT_BIND_SRC);
    let explicit = build_func_body(&VARIANT_BIND_SRC.replace("B(val) := v", "B(val): int = v"));
    assert_eq!(infer, explicit);
}
