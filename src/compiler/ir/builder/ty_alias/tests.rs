use super::*;
use ast::TyNode;

fn alias(name: &str, base: TyNode) -> ast::Group1Node {
    ast::TypeAlias::new(name.to_string(), base, test_span())
}

fn int() -> TyNode {
    TyNode::Ty("int".to_string())
}

#[test]
fn chain_is_fully_expanded() {
    let nodes = vec![alias("A", TyNode::Ty("B".into())), alias("B", int())];
    let map = map_alias(&nodes).unwrap();
    assert_eq!(map["A"], int());
    assert_eq!(map["B"], int());
}

#[test]
fn pointer_inside_is_expanded() {
    let nodes = vec![alias("T", int())];
    let map = map_alias(&nodes).unwrap();
    let mut t = TyNode::Pointer {
        is_const: false,
        ty_name: Box::new(TyNode::Ty("T".into())),
        range: None,
    };
    ty(&mut t, &map);
    assert_eq!(
        t,
        TyNode::Pointer {
            is_const: false,
            ty_name: Box::new(int()),
            range: None
        }
    );
}

#[test]
fn cycle_is_error() {
    let nodes = vec![
        alias("A", TyNode::Ty("B".into())),
        alias("B", TyNode::Ty("A".into())),
    ];
    assert!(map_alias(&nodes).is_err());
    assert!(map_alias(&[alias("T", TyNode::Ty("T".into()))]).is_err());
}

#[test]
fn duplicate_is_error() {
    let nodes = vec![alias("T", int()), alias("T", int())];
    assert!(map_alias(&nodes).is_err());
}
