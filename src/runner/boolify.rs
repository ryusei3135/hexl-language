use super::*;


pub fn node_to_bool(node: node::CalculNode) -> bool {
    match eval::node_run(node) {
        type_info::VarValue::Int32(v) => v != 0,
        type_info::VarValue::Bool(v) => v,
        _ => false,
    }
}
