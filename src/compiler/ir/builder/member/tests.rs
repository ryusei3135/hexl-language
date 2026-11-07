use super::*;
use crate::compiler::ast::{Field, StructDefine, StructField, TyNode};

#[test]
fn private_fields_are_accessible_only_from_their_struct() {
    let mut ir = super::super::new_test_ir();
    let mut public_field = StructField::new(
        "public_field".to_string(),
        TyNode::Ty("int".to_string()),
        Rc::clone(&ir.current_span),
    );
    public_field.public = true;
    let Group1Node::StructDefine(def) = StructDefine::new(
        "Record".to_string(),
        vec![
            public_field,
            StructField::new(
                "private_field".to_string(),
                TyNode::Ty("int".to_string()),
                Rc::clone(&ir.current_span),
            ),
        ],
        Vec::new(),
        Rc::clone(&ir.current_span),
    ) else {
        unreachable!();
    };
    ir.struct_tree.add(&def);

    assert!(
        ir.ensure_struct_field_access("Record", "public_field")
            .is_ok()
    );
    let error = ir
        .ensure_struct_field_access("Record", "private_field")
        .unwrap_err();
    let err::ErrKind::CompileErr(error) = error else {
        panic!("private field access should return a compile error");
    };
    assert_eq!(error.kind.title(), "これはプライベートです");

    ir.active_struct = Some("Record".to_string());
    assert!(
        ir.ensure_struct_field_access("Record", "private_field")
            .is_ok()
    );
}
