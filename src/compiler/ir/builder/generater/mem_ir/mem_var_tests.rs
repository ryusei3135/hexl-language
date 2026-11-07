use super::*;

#[test]
fn check_stack_struct_inst() {
    let body = build_func_body("_start(): int { a: [int] = 100 }");
    let name = String::from("a");
    assert!(
        body.iter().any(|inst| {
            if let inst::Inst::MemoryValue(inst::MemoryInst::Memory {
                name: mem_name,
                size,
                src,
                kind,
                dst,
            }) = inst
            {
                mem_name == &name
                    && *size == types::Size::DD
                    && src == &vec![0]
                    && *kind == inst::MemoryKind::Stack
                    && *dst == 1
            } else {
                false
            }
        }),
        "{:?}",
        body
    );
}

/*#[test]
fn check_stack_array_struct_inst() {
    let body = build_func_body("_start(): int { a: [int 4] = [100, 100, 100, 100] }");
    assert!(
        body.iter().any(|inst| matches!(
            inst,
            inst::Inst::Struct(v) if v == &vec![
                inst::MemoryInst::Stack,
                inst::MemoryInst::Byte(4),
                inst::MemoryInst::Byte(4),
                inst::MemoryInst::Byte(4),
                inst::MemoryInst::Byte(4),
            ]
        )),
        "{:?}", body
    );
}*/

#[test]
fn check_static_struct_inst() {
    let body = build_func_body("_start(): int { a: static[int] = 100 }");
    let name = String::from("a");
    assert!(
        body.iter().any(|inst| {
            if let inst::Inst::MemoryValue(inst::MemoryInst::Memory {
                name: mem_name,
                size,
                src,
                kind,
                dst,
            }) = inst
            {
                mem_name == &name
                    && *size == types::Size::DD
                    && src == &vec![0]
                    && *kind == inst::MemoryKind::Static
                    && *dst == 1
            } else {
                false
            }
        }),
        "{:?}",
        body
    );
}

#[test]
fn check_variant_enum_like_as_ty_and_access() {
    // 型なしメンバーだけの`variant`(旧列挙型)を型として使い、
    // `Name::Mem`でメンバにアクセスできる
    let body = build_func_body(
        "variant Color { Red Green Blue } _start(): int { a: Color = Color::Green }",
    );
    assert!(
        body.iter().any(|inst| matches!(
            inst,
            inst::Inst::Num { value, .. } if value == "1"
        )),
        "Color::Green は2番目(index 1)のメンバとして展開される必要がある: {:?}",
        body
    );
}

#[test]
fn check_variant_enum_like_forward_reference() {
    // `variant`が使われる場所より後に定義されていても解決できる
    let body = build_func_body(
        "_start(): int { a: Color = Color::Blue } variant Color { Red Green Blue }",
    );
    assert!(
        body.iter().any(|inst| matches!(
            inst,
            inst::Inst::Num { value, .. } if value == "2"
        )),
        "{:?}",
        body
    );
}

#[test]
fn check_combined_struct_variant_program() {
    // 構造体と`variant`を同じプログラム内で型として使えることを確認する
    let body = build_func_body(
        "variant Color { Red Green Blue } struct Point { x: int y: int } _start(): int { c: Color = Color::Green p: Point = Point { x: 1 y: 2 } }",
    );
    assert!(
        body.iter()
            .any(|i| matches!(i, inst::Inst::Num{value, ..} if value == "1"))
    );
    assert!(body.iter().any(|i| matches!(
        i,
        inst::Inst::Struct { mem, .. } if mem.len() == 2
    )));
}

#[test]
fn check_variant_with_data_init() {
    // データ付きメンバーの初期化は、タグ付きの`Inst::Variant`になる
    let body = build_func_body("variant V { A B(int) } _start(): int { v: V = V { B: 5 } }");
    assert!(
        body.iter().any(|i| matches!(
            i,
            inst::Inst::Variant {
                tag: 1,
                value: Some(_),
                tagged: true,
                ..
            }
        )),
        "{:?}",
        body
    );
}

#[test]
fn check_variant_typeless_member_in_data_variant() {
    // データ付きメンバーを持つ`variant`の型なしメンバーは`Inst::Variant`(値なし)
    let body = build_func_body("variant V { A B(int) } _start(): int { v: V = V::A }");
    assert!(
        body.iter().any(|i| matches!(
            i,
            inst::Inst::Variant {
                tag: 0,
                value: None,
                tagged: true,
                ..
            }
        )),
        "{:?}",
        body
    );
}

#[test]
fn check_generic_variant_infers_type_from_context() {
    let body = build_func_body(
        "variant Option<T> { Some(T) None } create(): Option<int> { ret Option::None } _start(): int { value: int = 10 some: Option<int> = Option::Some(value) none: Option<int> = Option::None }",
    );
    let variants = body
        .iter()
        .filter_map(|inst| match inst {
            inst::Inst::Variant { name, tag, .. } => Some((name.as_str(), tag)),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(variants.contains(&("Option$int", &0)), "{:?}", body);
    assert!(variants.contains(&("Option$int", &1)), "{:?}", body);
}

#[test]
fn check_variant_unsafe_has_no_tag() {
    // `"unsafe"`はタグを持たない
    let body = build_func_body(
        "variant \"unsafe\" V { a: int b: i64 } _start(): int { v: V = V { b: 5 } }",
    );
    assert!(
        body.iter().any(|i| matches!(
            i,
            inst::Inst::Variant {
                tagged: false,
                value: Some(_),
                ..
            }
        )),
        "{:?}",
        body
    );
}

#[test]
fn check_variant_size() {
    // タグ(4バイト) + 最大のメンバー(i64 = 8バイト)
    let src = "variant V { A B(int) C(i64) } _start(): int { v: V = V { C: 1 } }";
    let body = build_func_body(src);
    let size = body.iter().find_map(|i| match i {
        inst::Inst::Variant { size, .. } => Some(size.clone()),
        _ => None,
    });
    assert_eq!(size.map(|s| s.to_bytes()), Some(12), "{:?}", body);
}

#[test]
fn check_struct_as_ty_and_init() {
    // 構造体を型として使い、フィールドを初期化できる
    let body = build_func_body(
        "struct Point { x: int y: int } _start(): int { p: Point = Point { x: 1 y: 2 } }",
    );
    assert!(
        body.iter().any(|inst| matches!(
            inst,
            inst::Inst::Struct { mem: members, .. } if members.len() == 2
                && members.iter().any(|m| matches!(
                    m,
                    inst::MemoryInst::Member { parent, .. } if parent == "x"
                ))
                && members.iter().any(|m| matches!(
                    m,
                    inst::MemoryInst::Member { parent, .. } if parent == "y"
                ))
        )),
        "{:?}",
        body
    );
}
