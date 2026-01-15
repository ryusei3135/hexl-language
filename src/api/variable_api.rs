use super::*;


pub fn get_variable_info(name: String) -> variable::VariableInfo {
    global_state::var_manager().get_var(name)
}

pub fn update_variable_value(name: String, new_value: node::CalculNode) {
    if !global_state::var_manager().update_var(name, new_value) {
        std::process::exit(1);
    }
}

pub fn call_var_value(name: String) -> String {
    let var_info = get_variable_info(name);
    return run::node_run(var_info.value);
}

pub fn define_var(node: node::CalculNode) -> String {
    let var_name = node.value;
    let assign_value = node::CalculNode {
        value: run::node_run(*node.right_node.unwrap()),
        node_type: node::NodeKind::NodeNum,
        left_node: None,
        right_node: None,
        //  どこのブロックの中か
        block: node.block,
    };
    let var_type = node.left_node.unwrap().value;
    global_state::var_manager().add_var(
        var_name.clone(),
        assign_value,
        var_type,
    );
    return call_var_value(var_name.clone());
}

//  変数の値の上書き
pub fn update_var_value(node: node::CalculNode) -> String {
    if node.left_node.clone().unwrap().node_type == node::NodeKind::NodeVarName {
        let var_name = node.left_node.unwrap().value.clone();
        //  代入する値を計算
        let assign_value = node::CalculNode {
            value: run::node_run(*node.right_node.unwrap()),
            node_type: node::NodeKind::NodeNum,
            left_node: None,
            right_node: None,
            //  どこのブロックの中か
            block: node.block,
        };
        global_state::var_manager().update_var(
            var_name.clone(),
            assign_value
        );
        return call_var_value(var_name.clone());
    } else {
        println!("syntax err: assign var");
        panic!("assign var");
    }
}
