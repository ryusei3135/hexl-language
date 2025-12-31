use crate::parse::node;
use crate::manager::global_state::func_manager;
use crate::runner::storage::var;


fn get_left_value(node: &node::CalculNode) -> i32 {
    node_run(*node.left_node.clone().unwrap()).parse::<i32>().unwrap()
}

fn get_right_value(node: &node::CalculNode) -> i32 {
    node_run(*node.right_node.clone().unwrap()).parse::<i32>().unwrap()
}

pub fn node_run(
        node: node::CalculNode,
) -> String {
    match node.node_type {
        node::NodeKind::NodeNum => node.value.clone(),
        node::NodeKind::NodeNot => (!get_left_value(&node)).to_string(),
        node::NodeKind::NodeStr => node.value.clone(),
        node::NodeKind::NodeAdd => (get_left_value(&node) + get_right_value(&node)).to_string(),
        node::NodeKind::NodeSub => (get_left_value(&node) - get_right_value(&node)).to_string(),
        node::NodeKind::NodeMul => (get_left_value(&node) * get_right_value(&node)).to_string(),
        node::NodeKind::NodeDiv => (get_left_value(&node) / get_right_value(&node)).to_string(),
        node::NodeKind::NodeEqTo => (get_left_value(&node) == get_right_value(&node)).to_string(),
        node::NodeKind::NodeNotEqTo => (get_left_value(&node) != get_right_value(&node)).to_string(),
        node::NodeKind::NodeAssignVar => var::update_var_value(node.clone()),
        node::NodeKind::NodeCallVar => var::call_var_value(node.value),
        node::NodeKind::NodeDefVar => var::define_var(node.clone()),
        node::NodeKind::NodeCallFunc => run_func(func_manager().get_func(&node.value.clone())),
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
