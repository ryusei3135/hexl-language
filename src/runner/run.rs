use crate::parse::node;
use crate::manager::global_state::func_manager;
use crate::runner::storage::var;
use crate::runner::runtime;


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
        node::NodeKind::NodeCallFunc => {
            let func_data = func_manager().get_func(&node.value.clone());
            var::make_args_var(func_data.clone().args.clone(), *node.left_node.unwrap().clone());
            run_func(func_data)
        }
        node::NodeKind::NodeIf => {
            runtime::process_kind(node::NodeKind::NodeIf);
            node_run(*node.left_node.unwrap().clone())
        }
        node::NodeKind::NodeIfElse => {
            runtime::process_kind(node::NodeKind::NodeIfElse);
            node_run(*node.left_node.unwrap().clone())
        }
        node::NodeKind::NodeElse => {
            runtime::process_kind(node::NodeKind::NodeElse);
            "1".to_string()
        }
        node::NodeKind::NodeRet => {
            runtime::process_kind(node::NodeKind::NodeRet);
            node_run(*node.left_node.unwrap().clone())
        }
        _ => {
            String::new()
        }
    }
}

fn run_func(func_process: node::FuncNode) -> String {
    let mut cond_status = runtime::CondStatus::new();
    let mut index: u32 = 0;
    let mut executable_area: i32 = 1;

    while func_process.nodes.len() > index as usize {
        let now_area = func_process.nodes[index as usize].block.unwrap();

        if now_area == executable_area {
            let result = node_run(func_process.nodes[index as usize].clone());

            match runtime::get_process_kind() {
                node::NodeKind::NodeIf => {
                    runtime::process_kind(node::NodeKind::NodeNull);
                    if result.parse().unwrap() {
                        //  trueを代入すると、つながっている条件分岐がスキップされる
                        cond_status.push(true, now_area);
                        executable_area = func_process.nodes[1 + index as usize].block.unwrap();
                    } else {
                        cond_status.push(false, now_area);
                    }
                }
                node::NodeKind::NodeIfElse => {
                    runtime::process_kind(node::NodeKind::NodeNull);
                    if result.parse().unwrap() {
                        if cond_status.judge_cond(now_area) {
                            cond_status.cond_true();
                            executable_area = func_process.nodes[1 + index as usize].block.unwrap();
                        }
                    }
                }
                node::NodeKind::NodeElse => {
                    runtime::process_kind(node::NodeKind::NodeNull);
                    if cond_status.judge_cond(now_area) {
                        //  else文なので、条件のデータを削除する
                        cond_status.del();
                        executable_area = func_process.nodes[1 + index as usize].block.unwrap();
                    }
                }
                node::NodeKind::NodeRet => {
                    runtime::process_kind(node::NodeKind::NodeNull);
                    return result;
                }
                _ => println!("[{}] [value]", result),
            }
        } else if now_area < executable_area {
            executable_area = now_area;
        }
        index += 1;
    }
    "[*end*]".to_string()
}


pub fn start_process() {
    let start_process = func_manager().get_func("start");

    run_func(start_process);
}
