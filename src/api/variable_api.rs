use super::*;


pub fn get_variable_info(name: String) -> variable::VariableInfo {
    global_state::var_manager().get_var(name)
}

pub fn update_variable_value(name: String, new_value: type_info::VarValue) {
    if !global_state::var_manager().update_var(name, new_value) {
        std::process::exit(1);
    }
}

pub fn call_var_value(name: String) -> type_info::VarValue {
    let var_info = get_variable_info(name);
    var_info.value
}

pub fn define_var(node: node::CalculNode) -> type_info::VarValue {
    let var_name = node.value;

    let value = run::node_run(*node.right_node.clone().unwrap());

    let var_type = node.left_node.unwrap().value;
    global_state::var_manager().add_var(
        var_name.clone(),
        value,
        var_type,
    );
    return call_var_value(var_name.clone());
}

//  変数の値の上書き
pub fn update_var_value(node: node::CalculNode) -> type_info::VarValue {
    if node.left_node.clone().unwrap().node_type == node::NodeKind::NodeVarName {
        let var_name = node.left_node.unwrap().value.clone();
        let value = run::node_run(*node.right_node.unwrap());

        global_state::var_manager().update_var(
            var_name.clone(),
            value,
        );
        return call_var_value(var_name.clone());
    } else {
        println!("syntax err: assign var");
        panic!("assign var");
    }
}
