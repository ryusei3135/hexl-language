//! インタプリのパイプライン

use super::*;

/// インタプリを実行するパイプライン
pub(super) fn run_func(func_process: func::FuncNode) -> type_info::VarValue {
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
            let result = eval::node_run(func_process.nodes[index].clone());

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
                                log::output_log_L0(log);
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
                        crate::branch_if_cond_flag!(
                            func_process.nodes[index].node_type.clone(),
                            cond_status
                        );
                        executable_area = now_area;
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

/// エントリーポイントを呼び出す
pub fn start_process() {
    let start_process = func_manager().get_func("start");

    run_func(start_process);
}
