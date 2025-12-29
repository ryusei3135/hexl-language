
use crate::manager::variable::VariableManager;
use crate::manager::variable::VariableData;
use crate::runner::run;
use crate::parse::node;
use crate::parse::resp;


//  変数を呼び出す
pub fn call_var_value(name: String, variable_data: &mut VariableManager) -> String {
    let var_data = variable_data.get_var(name);
    return run::node_run(var_data.value, variable_data);
}

//  変数の値の上書き
pub fn update_var_value(node: node::CalculNode, variable_data: &mut VariableManager) -> String {
    if node.left_node.clone().unwrap().node_type == node::NodeKind::NodeVarName {
        let var_name = node.left_node.unwrap().value.clone();
        //  代入する値を計算
        let assign_value = resp::handler::convert_value_to_node(
            run::node_run(*node.right_node.unwrap(), variable_data),
            node::NodeKind::NodeNum,
        );
        variable_data.update_var(
            var_name.clone(),
            assign_value
        );
        return call_var_value(var_name.clone(), variable_data);
    } else {
        println!("syntax err: assign var");
        panic!("assign var");
    }
}

//  変数の定義
pub fn define_var(node: node::CalculNode, variable_data: &mut VariableManager) -> String {
    let var_name = node.value;
    let assign_value = resp::handler::convert_value_to_node(
        run::node_run(*node.right_node.unwrap(), variable_data),
        node::NodeKind::NodeNum,
    );
    let var_type = node.left_node.unwrap().value;
    variable_data.add_var(
        var_name.clone(),
        assign_value,
        var_type
    );
    return call_var_value(var_name.clone(), variable_data);
}