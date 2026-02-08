//! インタプリのノードを実行する

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
        node::NodeKind::NodeRangeOp => {
            match (get_left_value(&node), get_right_value(&node)) {
                (type_info::VarValue::Int32(l), type_info::VarValue::Int32(r)) => {
                    let mut arr = Vec::<Box<type_info::VarValue>>::new();
                    for v in l..r {
                        arr.push(Box::new(type_info::VarValue::Int32(v)));
                    }
                    type_info::VarValue::Array(arr)
                }
                _ => panic!("node range err"),
            }
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
            let func_data = func_manager().get_func(&node.value);
            run::run_func(func_data, &Some(*node.left_node.unwrap()))
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
        node::NodeKind::NodeRet => type_info::VarValue::Flag(node::NodeKind::NodeRet),
        flag => {
            match flag_switch::set_runtime_flag(&flag) {
                Ok(_) => type_info::VarValue::Flag(flag),
                Err(_) => panic!("::"),
            }
        }
    }
}
