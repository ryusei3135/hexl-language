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
        block: None
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
        block: None
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
        block: None
    };
}

pub fn make_null_node() -> node::CalculNode {
    return node::CalculNode {
        value: "[*null*]".to_string(),
        node_type: node::NodeKind::NodeNull,
        left_node: None,
        right_node: None,
        block: None
    };
}

pub fn make_true_node(kind: node::NodeKind) -> node::CalculNode {
    return node::CalculNode {
        value: "1".to_string(),
        node_type: kind,
        left_node: None,
        right_node: None,
        block: None
    }
}

pub fn make_method_node(name: String, method_type: node::NodeKind) -> node::CalculNode {
    return node::CalculNode {
        value: name,
        node_type: method_type,
        left_node: None,
        right_node: None,
        block: None,
    }
}

pub fn make_receiver_node(name: String, method: node::CalculNode) -> node::CalculNode {
    return node::CalculNode {
        value: name,
        node_type: node::NodeKind::NodeReceiver,
        left_node: Some(Box::new(method)),
        right_node: None,
        block: None,
    };
}
