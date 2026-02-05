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
            flags::syn_flag::control_syn_flag(node::NodeKind::NodeIf);
            type_info::VarValue::Null(true)
        }
        node::NodeKind::NodeIfElse => {
            flags::syn_flag::control_syn_flag(node::NodeKind::NodeIfElse);
            type_info::VarValue::Null(true)
        }
        node::NodeKind::NodeElse => {
            flags::syn_flag::control_syn_flag(node::NodeKind::NodeElse);
            type_info::VarValue::Null(true)
        }
        node::NodeKind::NodeFor => {
            flags::syn_flag::control_syn_flag(node::NodeKind::NodeFor);
            type_info::VarValue::Null(true)
        }
        node::NodeKind::NodeRet => {
            flags::syn_flag::control_syn_flag(node::NodeKind::NodeRet);
            node_run(*node.left_node.unwrap().clone())
        }
        _ => panic!("node run 2"),
    }
}

fn run_func(func_process: func::FuncNode) -> type_info::VarValue {
    global_state::var_manager().make_new_stack();
    let mut cond_status = flags::handle_flag::ControlSynFlag::new();
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

            match flags::syn_flag::get_control_syn_flag() {
                node::NodeKind::NodeIf => {
                    flags::syn_flag::control_syn_flag(node::NodeKind::NodeNull);
                    if boolify::node_to_bool(*func_process.nodes[index].left_node.clone().unwrap()) {
                        //  trueを代入すると、つながっている条件分岐がスキップされる
                        cond_status.make_new_flag(true, now_area, node::NodeKind::NodeIf);
                        executable_area = func_process.nodes[1 + index].block.unwrap();
                    } else {
                        cond_status.make_new_flag(false, now_area, node::NodeKind::NodeIf);
                    }
                }
                node::NodeKind::NodeIfElse => {
                    flags::syn_flag::control_syn_flag(node::NodeKind::NodeNull);
                    if boolify::node_to_bool(*func_process.nodes[index].left_node.clone().unwrap()) {
                        if cond_status.judge_cond(now_area) {
                            cond_status.cond_status_true();
                            executable_area = func_process.nodes[1 + index].block.unwrap();
                        }
                    }
                }
                node::NodeKind::NodeElse => {
                    flags::syn_flag::control_syn_flag(node::NodeKind::NodeNull);
                    if cond_status.judge_cond(now_area) {
                        executable_area = func_process.nodes[1 + index].block.unwrap();
                    }
                }
                node::NodeKind::NodeFor => {
                    flags::syn_flag::control_syn_flag(node::NodeKind::NodeNull);
                    cond_status.make_new_flag(true, now_area, node::NodeKind::NodeFor);
                    // for文の設定をする
                    var_manager().make_new_stack();
                    match cond_status.now_loop(
                        Some(index.try_into().unwrap()),
                        Some(*func_process.nodes[index].left_node.clone().unwrap())
                    ) {
                        Ok(_) => {/* そもそもここで、Okが帰ってくることはない */},
                        Err(log) => {
                            if log != control_syn::ControlSynErr::SETTING {
                                result::output_log(log);
                            } else {
                                executable_area = func_process.nodes[1 + index].block.unwrap();
                            }
                        }
                    }
                }
                node::NodeKind::NodeRet => {
                    global_state::var_manager().remove_stack();
                    flags::syn_flag::control_syn_flag(node::NodeKind::NodeNull);
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
            if let Some(flag) = cond_status.get_now_flag() {
                match flag {
                    node::NodeKind::NodeIf => {
                        executable_area = now_area;
                        cond_status.del();
                        continue;
                    }
                    node::NodeKind::NodeFor => {
                        crate::update_array_index!(index, cond_status);
                    }
                    _ => panic!("[err: run func run]"),
                }
            } else {
                eprintln!("[what?]");
            }
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
