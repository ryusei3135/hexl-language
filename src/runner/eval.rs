//! インタプリのノードを実行する

use super::*;

fn call_method(
        runtime: &mut run::Runtime,
        receiver_name: String,
        method: variable::MethodInfo,
        call_value: node::CalculNode
) -> Option<type_info::VarValue> {
    return match method.node.node_type {
        node::NodeKind::NodeNativeFunc => {
            manage::run_native_func(runtime, &receiver_name, &method.name, &*call_value.left_node.clone().unwrap())
        }
        _ => None,
    };
}

fn call_module(
        runtime: &mut run::Runtime,
        module_name: &String,
        call_value: &node::CalculNode
) -> Option<type_info::VarValue> {
    manage::run_native_func(
        runtime,
        module_name,
        &call_value.value,
        &*call_value.left_node.clone().unwrap()
    )
}

fn get_left_value(
        runtime: &mut run::Runtime,
        bind_type: &Option<String>,
        node: &node::CalculNode
) -> type_info::VarValue {
    node_run(runtime, bind_type, *node.left_node.clone().unwrap())
}

fn get_right_value(
        runtime: &mut run::Runtime,
        bind_type: &Option<String>,
        node: &node::CalculNode
) -> type_info::VarValue {
    node_run(runtime, bind_type, *node.right_node.clone().unwrap())
}


pub fn node_run(
        runtime: &mut run::Runtime,
        bind_type: &Option<String>,
        node: node::CalculNode
) -> type_info::VarValue {
    match node.node_type {
        node::NodeKind::NodeNum => {
            if let Some(type_name) = bind_type {
                match type_api::change_txt_type_to_type(type_name) {
                    type_info::VarType::Int32 => {
                        if node.value.clone().parse::<i32>().is_ok() {
                            return type_info::VarValue::Int32(node.value.clone().parse::<i32>().unwrap());
                        }
                    }
                    _ => panic!("unmatch var type"),
                }
            } else {
                if node.value.clone().parse::<i32>().is_ok() {
                    return type_info::VarValue::Int32(node.value.clone().parse::<i32>().unwrap());
                }
            }
            panic!("[system err] This sequence cannot be classified into any category");
        }
        node::NodeKind::NodeBoolTrue => {
            if let Some(type_name) = bind_type {
                if type_api::change_txt_type_to_type(type_name) == type_info::VarType::Bool {
                    return type_info::VarValue::Bool(true);
                }
                panic!("[system err] This sequence cannot be classified into any category");
            } else {
                return type_info::VarValue::Bool(true);
            }
        }
        node::NodeKind::NodeBoolFalse => {
            if let Some(type_name) = bind_type {
                if type_api::change_txt_type_to_type(type_name) == type_info::VarType::Bool {
                    return type_info::VarValue::Bool(false);
                }
                panic!("[system err] This sequence cannot be classified into any category");
            } else {
                return type_info::VarValue::Bool(false);
            }
        }
        node::NodeKind::NodeFloat => {
            if let Some(type_name) = bind_type {
                if type_api::change_txt_type_to_type(type_name) == type_info::VarType::Float32 {
                    if node.value.clone().parse::<f32>().is_ok() {
                        return type_info::VarValue::Float32(node.value.clone().parse::<f32>().unwrap());
                    }
                }
                panic!("[system err] This sequence cannot be classified into any category");
            } else {
                if node.value.clone().parse::<f32>().is_ok() {
                    return type_info::VarValue::Float32(node.value.clone().parse::<f32>().unwrap());
                }
                panic!("[system err] This sequence cannot be classified into any category");
            }
        }
        node::NodeKind::NodeRangeOp => {
            match (get_left_value(runtime, bind_type, &node), get_right_value(runtime, bind_type, &node)) {
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
            let array_value = expand::expand_array_node(runtime, node);
            return type_info::VarValue::Array(array_value.to_vec());
        }
        node::NodeKind::NodeNot => {
            match get_left_value(runtime, bind_type, &node) {
                type_info::VarValue::Int32(v) => type_info::VarValue::Bool(!(v != 0)),
                type_info::VarValue::Float32(v) => type_info::VarValue::Bool(!(v != 0.0)),
                type_info::VarValue::Bool(v) => type_info::VarValue::Bool(!v),
                _ => panic!("err not node"),
            }
        }
        node::NodeKind::NodeStr => {
            if let Some(type_name) = bind_type {
                if type_api::change_txt_type_to_type(type_name) == type_info::VarType::Str {
                    return type_info::VarValue::Str(node.value.clone());
                }
                panic!("[system err] This sequence cannot be classified into any category");
            } else {
                return type_info::VarValue::Str(node.value.clone());
            }
        }
        node::NodeKind::NodeAdd => {
            crate::calcul_by_type!(
                +,
                bind_type,
                runtime,
                node,
                type_info::VarValue::Int32,
                type_info::VarValue::Float32
            )
        }
        node::NodeKind::NodeSub => {
            crate::calcul_by_type!(
                -,
                bind_type,
                runtime,
                node,
                type_info::VarValue::Int32,
                type_info::VarValue::Float32
            )
        }
        node::NodeKind::NodeMul => {
            crate::calcul_by_type!(
                *,
                bind_type,
                runtime,
                node,
                type_info::VarValue::Int32,
                type_info::VarValue::Float32
            )
        }
        node::NodeKind::NodeDiv => {
            crate::calcul_by_type!(
                /,
                bind_type,
                runtime,
                node,
                type_info::VarValue::Int32,
                type_info::VarValue::Float32
            )
        }
        node::NodeKind::NodeModulo => {
            crate::calcul_by_type!(
                %,
                bind_type,
                runtime,
                node,
                type_info::VarValue::Int32,
                type_info::VarValue::Float32
            )
        }
        node::NodeKind::NodeEqTo => {
            crate::comper_op_type!(
                ==,
                bind_type,
                runtime,
                node,
                type_info::VarValue::Int32,
                type_info::VarValue::Float32
            )
        }
        node::NodeKind::NodeNotEqTo => {
            crate::comper_op_type!(
                !=,
                bind_type,
                runtime,
                node,
                type_info::VarValue::Int32,
                type_info::VarValue::Float32
            )
        }
        node::NodeKind::NodeLessThanOrEqualTo => {
            crate::comper_op_type!(
                <=,
                bind_type,
                runtime,
                node,
                type_info::VarValue::Int32,
                type_info::VarValue::Float32
            )
        }
        node::NodeKind::NodeGreaterThanOrEqualTo => {
            crate::comper_op_type!(
                >=,
                bind_type,
                runtime,
                node,
                type_info::VarValue::Int32,
                type_info::VarValue::Float32
            )
        }
        node::NodeKind::NodeLessThan => {
            crate::comper_op_type!(
                <,
                bind_type,
                runtime,
                node,
                type_info::VarValue::Int32,
                type_info::VarValue::Float32
            )
        }
        node::NodeKind::NodeGreaterThan => {
            crate::comper_op_type!(
                >,
                bind_type,
                runtime,
                node,
                type_info::VarValue::Int32,
                type_info::VarValue::Float32
            )
        }
        node::NodeKind::NodeAssignVar => {
            match variable_api::update_var_value(runtime, node.clone()) {
                Ok(v) => v,
                Err(e) => {
                    e.print_log(&node.block.unwrap(), &"test".to_string());
                    panic!("");
                }
            }
        }
        node::NodeKind::NodeCallVar => {
            variable_api::call_var_value(runtime, &node.value)
        }
        node::NodeKind::NodeDefVar => {
            variable_api::define_var(runtime, &node)
        }
        node::NodeKind::NodeCallFunc => {
            match runtime.all_info.func_info.get_func(&node.value) {
                Ok(func_data) => {
                    match runtime.run_func(func_data, &Some(*node.left_node.unwrap())) {
                        Ok(v) => return v,
                        Err(e) => {
                            e.print_log(&node.block.unwrap(), &"rr".to_string());
                            panic!("JJJ");
                        }
                    }
                }
                Err(e) => {
                    e.print_log(&node.block.unwrap(), &"".to_string());
                    panic!("KK");
                }
            }
        }
        node::NodeKind::NodeReceiver => {
            let receiver_name = node.value.clone();
            //  引数や、メゾットの名前
            let method = *node.left_node.clone().unwrap();

            let method_data = runtime.all_info.var_info.get_method(receiver_name.clone(), method.value.clone());
            if method_data.name == method.value.clone() {
                if let Some(result) = call_method(runtime, receiver_name, method_data, method) {
                    return result;
                }
            }
            type_info::VarValue::Null(false)
        }
        node::NodeKind::NodeCallModule => {
            let module_name = node.value.clone();
            //  引数や、メゾットの名前
            let name = *node.left_node.clone().unwrap();

            if let Some(result) = call_module(runtime, &module_name, &name) {
                return result;
            }
            type_info::VarValue::Null(false)
        }
        node::NodeKind::NodeRet => type_info::VarValue::Flag(node::NodeKind::NodeRet),
        flag => {
            match flag_switch::set_runtime_flag(&flag) {
                Ok(_) => type_info::VarValue::Flag(flag),
                Err(e) => panic!("what flag {:?}", e),
            }
        }
    }
}
