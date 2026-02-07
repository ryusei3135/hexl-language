use super::*;


pub fn get_variable_info(name: String) -> Result<variable::VariableInfo, define_msg::VarErrorOrLog> {
    global_state::var_manager().get_var(name)
}

// pub fn update_variable_value(name: String, new_value: type_info::VarValue) {
//     if !global_state::var_manager().update_var(name, new_value) {
//         std::process::exit(1);
//     }
// }

pub fn call_var_value(name: String) -> type_info::VarValue {
    match get_variable_info(name) {
        Ok(var_info) => var_info.value,
        Err(_) => panic!("variable is not defined"),
    }
}

pub fn define_var(node: node::CalculNode) -> type_info::VarValue {
    let var_name = node.value;

    let value = eval::node_run(*node.right_node.clone().unwrap());

    global_state::var_manager().add_var(
        var_name.clone(),
        value,
        VarRegion::Stack,
    );
    call_var_value(var_name.clone())
}

//  変数の値の上書き
pub fn update_var_value(
        node: node::CalculNode
) -> Result<type_info::VarValue, define_msg::VarErrorOrLog> {
    if node.left_node.clone().unwrap().node_type == node::NodeKind::NodeVarName {
        let var_name = node.left_node.unwrap().value.clone();
        let value = eval::node_run(*node.right_node.unwrap());

        global_state::var_manager().update_var(
            var_name.clone(),
            value,
        );
        return Ok(call_var_value(var_name.clone()));
    } else {
        println!("syntax err: assign var");
    }
    return Err(define_msg::VarErrorOrLog::VarIsNotDefined);
}

fn setting_iter_status(
        loop_range: type_info::VarValue,
        bind_value: Option<String>
) -> Result<IterStatus, ControlSynErr> {
    return match loop_range {
        type_info::VarValue::Int32(end) => {
            init_iter_status(
                [
                    type_info::VarValue::Int32(0),
                    type_info::VarValue::Int32(end),
                ],
                bind_value,
            )
        }
        type_info::VarValue::Array(value) => {
            init_iter_status(
                [
                    type_info::VarValue::Array(value),
                    type_info::VarValue::Int32(0),
                ],
                bind_value,
            )
        }
        _ => {
            // for文で使えない型
            Err(ControlSynErr::ValueIsOfInvalidType)
        }
    };
}

pub fn update_loop_var(
        iter_now_status: &mut IterStatus,
) -> Result<(), ControlSynErr> {
    let loop_var = iter_now_status.loop_var.clone();

    match (iter_now_status.range.clone(), loop_var) {
        ([type_info::VarValue::Int32(_), type_info::VarValue::Int32(r)], type_info::VarValue::Int32(v)) => {
            if v + 1 < r {
                iter_now_status.loop_var = type_info::VarValue::Int32(v + 1);
            } else {
                // 範囲の外になったので、for文を終わらせる
                iter_now_status.executable = false;
            }
            Ok(())
        }
        ([type_info::VarValue::Array(arr), type_info::VarValue::Int32(c)], type_info::VarValue::Int32(_)) => {
            if arr.len() > c as usize {
                iter_now_status.loop_var = *arr[c as usize].clone();
                iter_now_status.range[1] = type_info::VarValue::Int32(c + 1);
            } else {
                iter_now_status.executable = false;
            }
            Ok(())
        }
        _ => Err(ControlSynErr::ValueIsOfInvalidType),
    }
}

/// for文の初期化やループ変数の更新のどをする関数
/// IterStatus は lang_api_type.rsを参照
/// # 引数
/// - loop_condはfor文の条件のノード
/// - now_valueは現在のfor文の状態(ループ変数の値など)
pub fn is_for_iterable(
        loop_cond: Option<node::CalculNode>,
        now_for_status: Option<IterStatus>,
) -> Result<IterStatus, ControlSynErr> {
    if let Some(cond) = loop_cond {
        match cond.node_type {
            node::NodeKind::NodeIn => {
                return match now_for_status {
                    Some(mut status) => {
                        update_loop_var(&mut status)?;
                        Ok(status)
                    }
                    None => {
                        setting_iter_status(
                            eval::node_run(*cond.left_node.clone().unwrap()),
                            Some(cond.value.clone())
                        )
                    }
                };
            }
            node::NodeKind::NodeNum => {
                return match now_for_status {
                    Some(mut status) => {
                        update_loop_var(&mut status)?;
                        Ok(status)
                    }
                    None => {
                        setting_iter_status(
                            eval::node_run(*cond.left_node.clone().unwrap()),
                            None,
                        )
                    }
                };
            }
            _ => return Err(ControlSynErr::ValueIsOfInvalidType),
        }
    } else {
        return match now_for_status {
            Some(mut status) => {
                update_loop_var(&mut status)?;
                Ok(status)
            }
            None => {
                Err(ControlSynErr::MissingCondInForStatement)
            }
        };
    }
}
