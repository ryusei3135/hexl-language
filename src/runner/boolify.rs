use super::*;


pub fn node_to_bool(
        runtime: &mut run::Runtime,
        node: node::CalculNode
) -> bool {
    match eval::node_run(runtime, &None, node) {
        type_info::VarValue::Int32(v) => v != 0,
        type_info::VarValue::Bool(v) => v,
        _ => false,
    }
}
