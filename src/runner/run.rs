use crate::parse::node;
use crate::parse::resp;
use crate::manager::func::FuncManager;
use crate::manager::variable::VariableManager;


pub fn node_run(
        node: node::CalculNode, 
        variable_data: &mut VariableManager
) -> String {
    match node.node_type {
        node::NodeKind::NodeNum => {
            node.value.clone()
        },
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
        node::NodeKind::NodeAssignVar => {
            if node.left_node.clone().unwrap().node_type == node::NodeKind::NodeVarName {
                let var_name = node.left_node.unwrap().value.clone();
                //  代入する値を計算
                let assign_value = resp::handler::convert_value_to_node(
                    node_run(*node.right_node.unwrap(), variable_data),
                    node::NodeKind::NodeNum,
                );
                variable_data.update_var(
                    var_name.clone(),
                    assign_value
                );
                return node_run(variable_data.get_var(var_name.clone()), variable_data);
            } else {
                println!("syntax err: assign var");
                panic!("assign var");
            }
        }
        node::NodeKind::NodeCallVar => {
            let var_name = node.value.clone();
            node_run(variable_data.get_var(var_name.clone()), variable_data)
        }
        node::NodeKind::NodeDefVar => {
            let var_name = node.value;
            let assign_value = resp::handler::convert_value_to_node(
                node_run(*node.right_node.unwrap(), variable_data),
                node::NodeKind::NodeNum,
            );
            variable_data.add_var(
                var_name.clone(),
                assign_value
            );
            return node_run(variable_data.get_var(var_name.clone()), variable_data);
        }
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