use crate::parse::node;
use crate::manager::func::FuncManager;
use crate::manager::variable::VariableManager;

use crate::runner::storage::var;


pub fn node_run(
        node: node::CalculNode, 
        variable_data: &mut VariableManager
) -> String {
    match node.node_type {
        node::NodeKind::NodeNum => {
            node.value.clone()
        },
        node::NodeKind::NodeStr => {
            node.value.clone()
        }
        node::NodeKind::NodeAdd => {
            let left_value = node_run(*node.left_node.unwrap(), variable_data);
            let right_value = node_run(*node.right_node.unwrap(), variable_data);
            let result = left_value.parse::<i32>().unwrap() + right_value.parse::<i32>().unwrap();
            result.to_string()
        },
        node::NodeKind::NodeSub => {
            let left_value = node_run(*node.left_node.unwrap(), variable_data);
            let right_value = node_run(*node.right_node.unwrap(), variable_data);
            let result = left_value.parse::<i32>().unwrap() - right_value.parse::<i32>().unwrap();
            result.to_string()
        },
        node::NodeKind::NodeMul => {
            let left_value = node_run(*node.left_node.unwrap(), variable_data);
            let right_value = node_run(*node.right_node.unwrap(), variable_data);
            let result = left_value.parse::<i32>().unwrap() * right_value.parse::<i32>().unwrap();
            result.to_string()
        },
        node::NodeKind::NodeDiv => {
            let left_value = node_run(*node.left_node.unwrap(), variable_data);
            let right_value = node_run(*node.right_node.unwrap(), variable_data);
            let result = left_value.parse::<i32>().unwrap() / right_value.parse::<i32>().unwrap();
            result.to_string()
        },
        node::NodeKind::NodeAssignVar => return var::update_var_value(node.clone(), variable_data),
        node::NodeKind::NodeCallVar => return var::call_var_value(node.value, variable_data),
        node::NodeKind::NodeDefVar => return var::define_var(node.clone(), variable_data),
        _ => {
            String::new()
        }
    }
}


pub fn start_process(func_datas: &FuncManager) {
    let mut variable_data = VariableManager::new();
    let start_process = func_datas.get_func("start");

    let mut index: i32 = 0;

    while start_process.nodes.len() > index as usize {
        println!(
            "[{}] [value]", 
            node_run(
                start_process.nodes[index as usize].clone(), 
                &mut variable_data
            )
        );

        index += 1;
    }
}