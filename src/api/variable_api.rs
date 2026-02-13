use super::*;

// pub fn update_variable_value(name: String, new_value: type_info::VarValue) {
//     if !global_state::var_manager().update_var(name, new_value) {
//         std::process::exit(1);
//     }
// }

/// 変数を呼び出す
/// # 引数
/// - runtime
///     インタプリタを実行するのに必要な物がすべて入っている
/// - name
///     呼び出す変数の名前
pub fn call_var_value(
        runtime: &mut run::Runtime,
        name: &String,
) -> type_info::VarValue {
    match runtime.all_info.var_info.get_var(name) {
        Ok(var_info) => var_info.value,
        Err(_) => panic!("variable is not defined"),
    }
}

fn search_var_type_node(node: &Option<Box<node::CalculNode>>) -> Option<String> {
    match node {
        Some(n) => {
            if n.node_type == node::NodeKind::NodeType {
                Some(n.value.clone())
            } else {
                None
            }
        }
        None => None,
    }
}

pub fn define_var(
        runtime: &mut run::Runtime,
        node: &node::CalculNode
) -> type_info::VarValue {
    let mut is_multiple: Option<variable::MultipleVar> = None;
    // もし変数定義に型があるなら、束縛する変数の名前をゲット
    let bind_type: Option<String> = match &node.left_node {
        Some(left) => {
            match left.node_type {
                node::NodeKind::NodeType => Some(left.value.clone()),
                node::NodeKind::NodeMut => {
                    is_multiple = Some(variable::MultipleVar::IsMut);
                    search_var_type_node(&left.left_node)
                }
                node::NodeKind::NodeImm => {
                    is_multiple = Some(variable::MultipleVar::IsImm);
                    search_var_type_node(&left.left_node)
                }
                _ => None,
            }
        }
        None => None,
    };

    let value = eval::node_run(runtime, &bind_type, *node.right_node.clone().unwrap());

    runtime.all_info.var_info.add_var(
        &node.value,
        value,
        &bind_type,
        VarRegion::Stack,
        is_multiple,
    );
    call_var_value(runtime, &node.value)
}

//  変数の値の上書き
pub fn update_var_value(
        runtime: &mut run::Runtime,
        node: node::CalculNode
) -> Result<type_info::VarValue, err_kind::ErrorsKind> {
    if node.left_node.clone().unwrap().node_type == node::NodeKind::NodeCallVar {
        let var_name = node.left_node.unwrap().value.clone();
        let value = eval::node_run(runtime, &None, *node.right_node.unwrap());

        runtime.all_info.var_info.update_var(
            &var_name,
            &value,
        )?;
        return Ok(call_var_value(runtime, &var_name));
    } else {
        println!("syntax err: assign var {:?}", node.left_node.clone().unwrap().node_type);
        return Err(err_kind::ErrorsKind::UndefinedVariable);
    }
}

/// for文の繰り返す条件を設定
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
        _ => Err(ControlSynErr::InvalidIterCond),
    }
}

/// for文の初期化やループ変数の更新のどをする関数
/// IterStatus は lang_api_type.rsを参照
/// # 引数
/// - loop_condはfor文の条件のノード
/// - now_valueは現在のfor文の状態(ループ変数の値など)
pub fn is_for_iterable(
        runtime: &mut run::Runtime,
        loop_cond: Option<node::CalculNode>,
        now_for_status: Option<IterStatus>,
) -> Result<IterStatus, ControlSynErr> {
    // for文の条件のノードが来たら、初期化をする
    // 違うならfor文のループ変数をアップデート
    if let Some(ref cond) = loop_cond {
        match cond.node_type {
            node::NodeKind::NodeIn => {
                // for文の情報があるなら、ループ変数を更新
                return match now_for_status {
                    Some(mut status) => {
                        update_loop_var(&mut status)?;
                        Ok(status)
                    }
                    None => {
                        // for文の情報がないので、for文の情報を設定
                        setting_iter_status(
                            eval::node_run(runtime, &None, *cond.left_node.clone().unwrap()),
                            Some(cond.value.clone())
                        )
                    }
                };
            }
            node::NodeKind::NodeNum | node::NodeKind::NodeRangeOp => {
                return match now_for_status {
                    Some(mut status) => {
                        update_loop_var(&mut status)?;
                        Ok(status)
                    }
                    None => {
                        setting_iter_status(
                            eval::node_run(runtime, &None, cond.clone()),
                            None,
                        )
                    }
                };
            }
            _ => return Err(ControlSynErr::ValueIsOfInvalidType),
        }
    } else {
        // ループ変数をアップデート
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
