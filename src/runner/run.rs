use crate::parse::node;
use crate::manager::global_state::func_manager;
use crate::runner::storage::var;


pub fn node_run(
        node: node::CalculNode,
) -> String {
    match node.node_type {
        node::NodeKind::NodeNum => {
            node.value.clone()
        },
        node::NodeKind::NodeStr => {
            node.value.clone()
        }
        node::NodeKind::NodeAdd => {
            let left_value = node_run(*node.left_node.unwrap());
            let right_value = node_run(*node.right_node.unwrap());
            let result = left_value.parse::<i32>().unwrap() + right_value.parse::<i32>().unwrap();
            result.to_string()
        },
        node::NodeKind::NodeSub => {
            let left_value = node_run(*node.left_node.unwrap());
            let right_value = node_run(*node.right_node.unwrap());
            let result = left_value.parse::<i32>().unwrap() - right_value.parse::<i32>().unwrap();
            result.to_string()
        },
        node::NodeKind::NodeMul => {
            let left_value = node_run(*node.left_node.unwrap());
            let right_value = node_run(*node.right_node.unwrap());
            let result = left_value.parse::<i32>().unwrap() * right_value.parse::<i32>().unwrap();
            result.to_string()
        },
        node::NodeKind::NodeDiv => {
            let left_value = node_run(*node.left_node.unwrap());
            let right_value = node_run(*node.right_node.unwrap());
            let result = left_value.parse::<i32>().unwrap() / right_value.parse::<i32>().unwrap();
            result.to_string()
        },
        node::NodeKind::NodeEqTo => {
            let left_value = node_run(*node.left_node.unwrap());
            let right_value = node_run(*node.right_node.unwrap());
            let result = left_value.parse::<i32>().unwrap() == right_value.parse::<i32>().unwrap();
            result.to_string()
        },
        node::NodeKind::NodeNotEqTo => {
            let left_value = node_run(*node.left_node.unwrap());
            let right_value = node_run(*node.right_node.unwrap());
            let result = left_value.parse::<i32>().unwrap() != right_value.parse::<i32>().unwrap();
            result.to_string()
        }
        node::NodeKind::NodeAssignVar => return var::update_var_value(node.clone()),
        node::NodeKind::NodeCallVar => return var::call_var_value(node.value),
        node::NodeKind::NodeDefVar => return var::define_var(node.clone()),
        node::NodeKind::NodeCallFunc => {
            return run_func(func_manager().get_func(&node.value.clone()));
        },
        _ => {
            String::new()
        }
    }
}

fn run_func(func_process: node::FuncNode) -> String {
    let mut index: u32 = 0;

    while func_process.nodes.len() > index as usize {
        println!("[{}] [value]", node_run(func_process.nodes[index as usize].clone()));
        index += 1;
    }
    "end".to_string()
}


pub fn start_process() {
    let start_process = func_manager().get_func("start");

    let mut index: u32 = 0;

    while start_process.nodes.len() > index as usize {
        println!(
            "[{}] [value] [{}]",
            node_run(start_process.nodes[index as usize].clone()),
            index
        );

        index += 1;
    }
}
