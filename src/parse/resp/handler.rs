use crate::token::token;

use crate::parse::node;


pub fn make_value_node(
        token: &token::Token,
        node_type: node::NodeKind
) -> node::CalculNode {
    node::CalculNode {
        value: token.lexeme.clone(),
        node_type: node_type,
        left_node: None,
        right_node: None,
    }
}

pub fn make_operator_node(
        left: node::CalculNode,
        right: node::CalculNode,
        node_type: node::NodeKind
) -> node::CalculNode {
    node::CalculNode {
        value: String::new(),
        node_type: node_type,
        left_node: Some(Box::new(left)),
        right_node: Some(Box::new(right)),
    }
}

pub fn convert_value_to_node(
        value: String,
        node_type: node::NodeKind
) -> node::CalculNode {
    return node::CalculNode {
        value: value,
        node_type: node_type,
        left_node: None,
        right_node: None,
    };
}

pub fn make_null_node() -> node::CalculNode {
    return node::CalculNode {
        value: "[*null*]".to_string(),
        node_type: node::NodeKind::NodeNull,
        left_node: None,
        right_node: None
    };
}
