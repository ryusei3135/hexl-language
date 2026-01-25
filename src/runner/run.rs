use crate::manager::global_state;

use super::*;


fn call_method(
        receiver_name: String,
        method: variable::MethodInfo,
        call_value: node::CalculNode
) -> Option<type_info::VarValue> {
    return match method.node.node_type {
        node::NodeKind::NodeNativeFunc => {
            manage::run_native_func(receiver_name, method.name, *call_value.left_node.clone().unwrap())
        }
        _ => None,
    };
}


fn get_left_value(node: &node::CalculNode) -> type_info::VarValue {
    node_run(*node.left_node.clone().unwrap())
}

fn get_right_value(node: &node::CalculNode) -> type_info::VarValue {
    node_run(*node.right_node.clone().unwrap())
}


pub fn node_run(
        node: node::CalculNode
) -> type_info::VarValue {
    match node.node_type {
        node::NodeKind::NodeNum => {
            if node.value.clone().parse::<i32>().is_ok() {
                return type_info::VarValue::Int32(node.value.clone().parse::<i32>().unwrap());
            }
            panic!("[system err] This sequence cannot be classified into any category");
        }
        node::NodeKind::NodeArray => {
            //  配列を展開
            let array_value = expand::expand_array_node(node);
            return type_info::VarValue::Array(array_value.to_vec());
        }
        node::NodeKind::NodeNot => {
            match get_left_value(&node) {
                type_info::VarValue::Int32(v) => type_info::VarValue::Bool(!(v != 0)),
                type_info::VarValue::Bool(v) => type_info::VarValue::Bool(!v),
                _ => panic!("err not node"),
            }
        }
        node::NodeKind::NodeStr => type_info::VarValue::Str(node.value.clone()),
        node::NodeKind::NodeAdd => {
            match (get_left_value(&node), get_right_value(&node)) {
                (type_info::VarValue::Int32(l), type_info::VarValue::Int32(r)) => type_info::VarValue::Int32(l + r),
                _ => panic!("node run 2 add"),
            }
        }
        node::NodeKind::NodeSub => {
            match (get_left_value(&node), get_right_value(&node)) {
                (type_info::VarValue::Int32(l), type_info::VarValue::Int32(r)) => type_info::VarValue::Int32(l - r),
                _ => panic!("node run 2 add"),
            }
        }
        node::NodeKind::NodeMul => {
            match (get_left_value(&node), get_right_value(&node)) {
                (type_info::VarValue::Int32(l), type_info::VarValue::Int32(r)) => type_info::VarValue::Int32(l * r),
                _ => panic!("node run 2 add"),
            }
        }
        node::NodeKind::NodeDiv => {
            match (get_left_value(&node), get_right_value(&node)) {
                (type_info::VarValue::Int32(l), type_info::VarValue::Int32(r)) => type_info::VarValue::Int32(l / r),
                _ => panic!("node run 2 add"),
            }
        }
        node::NodeKind::NodeEqTo => {
            match (get_left_value(&node), get_right_value(&node)) {
                (type_info::VarValue::Int32(l), type_info::VarValue::Int32(r)) => type_info::VarValue::Bool(l == r),
                _ => panic!("node run 2 add"),
            }
        }
        node::NodeKind::NodeNotEqTo => {
            match (get_left_value(&node), get_right_value(&node)) {
                (type_info::VarValue::Int32(l), type_info::VarValue::Int32(r)) => type_info::VarValue::Bool(l != r),
                _ => panic!("node run 2 add"),
            }
        }
        node::NodeKind::NodeLessThanOrEqualTo => {
            match (get_left_value(&node), get_right_value(&node)) {
                (type_info::VarValue::Int32(l), type_info::VarValue::Int32(r)) => type_info::VarValue::Bool(l <= r),
                _ => panic!("node run 2 add"),
            }
        }
        node::NodeKind::NodeGreaterThanOrEqualTo => {
            match (get_left_value(&node), get_right_value(&node)) {
                (type_info::VarValue::Int32(l), type_info::VarValue::Int32(r)) => type_info::VarValue::Bool(l >= r),
                _ => panic!("node run 2 add"),
            }
        }
        node::NodeKind::NodeLessThan => {
            match (get_left_value(&node), get_right_value(&node)) {
                (type_info::VarValue::Int32(l), type_info::VarValue::Int32(r)) => type_info::VarValue::Bool(l < r),
                _ => panic!("node run 2 add"),
            }
        }
        node::NodeKind::NodeGreaterThan => {
            match (get_left_value(&node), get_right_value(&node)) {
                (type_info::VarValue::Int32(l), type_info::VarValue::Int32(r)) => type_info::VarValue::Bool(l > r),
                _ => panic!("node run 2 add"),
            }
        }
        node::NodeKind::NodeAssignVar => {
            match variable_api::update_var_value(node.clone()) {
                Ok(v) => v,
                Err(_) => type_info::VarValue::Null(false),
            }
        }
        node::NodeKind::NodeCallVar => variable_api::call_var_value(node.value),
        node::NodeKind::NodeDefVar => variable_api::define_var(node.clone()),
        node::NodeKind::NodeCallFunc => {
            let func_data = func_manager().get_func(&node.value.clone());
            arg_api::make_args_var(func_data.clone().args.clone(), *node.left_node.unwrap().clone());
            run_func(func_data)
        }
        node::NodeKind::NodeReceiver => {
            let receiver_name = node.value.clone();
            //  引数や、メゾットの名前
            let method = *node.left_node.clone().unwrap();

            let method_data = var_manager().get_method(receiver_name.clone(), method.value.clone());
            if method_data.name == method.value.clone() {
                if let Some(result) = call_method(receiver_name, method_data, method) {
                    return result;
                }
            }
            type_info::VarValue::Null(false)
        }
        node::NodeKind::NodeIf => {
            runtime::process_kind(node::NodeKind::NodeIf);
            type_info::VarValue::Null(true)
        }
        node::NodeKind::NodeIfElse => {
            runtime::process_kind(node::NodeKind::NodeIfElse);
            type_info::VarValue::Null(true)
        }
        node::NodeKind::NodeElse => {
            runtime::process_kind(node::NodeKind::NodeElse);
            type_info::VarValue::Null(true)
        }
        node::NodeKind::NodeFor => {
            runtime::process_kind(node::NodeKind::NodeFor);
            type_info::VarValue::Null(true)
        }
        node::NodeKind::NodeRet => {
            runtime::process_kind(node::NodeKind::NodeRet);
            node_run(*node.left_node.unwrap().clone())
        }
        _ => panic!("node run 2"),
    }
}

fn run_func(func_process: func::FuncNode) -> type_info::VarValue {
    global_state::var_manager().make_new_stack();
    let mut cond_status = runtime::CondStatus::new();
    let mut index: usize = 0;
    let mut executable_area: usize = 1;

    while func_process.nodes.len() >= index {
        // 配列が最後の場所になったら、条件分岐や反復処理のどの制御構文がないか確認
        if func_process.nodes.len() == index {
            crate::update_array_index!(index, cond_status);
        }

        let now_area = func_process.nodes[index].block.unwrap();

        if now_area == executable_area {
            let result = node_run(func_process.nodes[index].clone());

            match runtime::get_process_kind() {
                node::NodeKind::NodeIf => {
                    runtime::process_kind(node::NodeKind::NodeNull);
                    if boolify::node_to_bool(*func_process.nodes[index].left_node.clone().unwrap()) {
                        //  trueを代入すると、つながっている条件分岐がスキップされる
                        cond_status.push(true, now_area);
                        executable_area = func_process.nodes[1 + index].block.unwrap();
                    } else {
                        cond_status.push(false, now_area);
                    }
                }
                node::NodeKind::NodeIfElse => {
                    runtime::process_kind(node::NodeKind::NodeNull);
                    if boolify::node_to_bool(*func_process.nodes[index].left_node.clone().unwrap()) {
                        if cond_status.judge_cond(now_area) {
                            cond_status.cond_true();
                            executable_area = func_process.nodes[1 + index].block.unwrap();
                        }
                    }
                }
                node::NodeKind::NodeElse => {
                    runtime::process_kind(node::NodeKind::NodeNull);
                    if cond_status.judge_cond(now_area) {
                        //  else文なので、条件のデータを削除する
                        cond_status.del();
                        executable_area = func_process.nodes[1 + index].block.unwrap();
                    }
                }
                node::NodeKind::NodeFor => {
                    runtime::process_kind(node::NodeKind::NodeNull);
                    cond_status.push(true, now_area);
                    let _ = cond_status.now_loop(
                        Some(index.try_into().unwrap()),
                        Some(*func_process.nodes[index].left_node.clone().unwrap())
                    );
                    executable_area = func_process.nodes[1 + index].block.unwrap();
                    crate::update_array_index!(index, cond_status);
                }
                node::NodeKind::NodeRet => {
                    global_state::var_manager().remove_stack();
                    runtime::process_kind(node::NodeKind::NodeNull);
                    return result;
                }
                _ => {
                    println!("[{}] [value]", match result {
                        type_info::VarValue::Int32(v) => v.to_string(),
                        type_info::VarValue::Bool(v) => v.to_string(),
                        type_info::VarValue::Str(v) => v,
                        _ => String::new(),
                    });
                }
            }
        } else if now_area < executable_area {
            crate::update_array_index!(index, cond_status);
            executable_area = now_area;
        }
        index += 1;
    }
    global_state::var_manager().remove_stack();
    type_info::VarValue::Null(false)
}


pub fn start_process() {
    let start_process = func_manager().get_func("start");

    run_func(start_process);
}
