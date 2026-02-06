//! インタプリのパイプライン
use super::*;

/// 条件分岐のフラグを立てるかどうかを制御
mod cond_branch_flag {
    use super::*;

    pub unsafe fn node_if(
            node: &Vec<node::CalculNode>,
            index: &usize,
            block_status: &mut [usize; 2],
            cond_status: &mut flags::handle_flag::ControlSynFlag,
    ) {
        cond_status.flag = node::NodeKind::NodeNull;
        if boolify::node_to_bool(*node[*index].left_node.clone().unwrap()) {
            //  trueを代入すると、つながっている条件分岐がスキップされる
            cond_status.make_new_flag(true, block_status[1], node::NodeKind::NodeIf);
            block_status[0] = node[1 + *index].block.unwrap();
        } else {
            cond_status.make_new_flag(false, block_status[1], node::NodeKind::NodeIf);
        }
    }

    pub unsafe fn node_if_else(
            node: &Vec<node::CalculNode>,
            index: &usize,
            block_status: &mut [usize; 2],
            cond_status: &mut flags::handle_flag::ControlSynFlag,
    ) {
        cond_status.flag = node::NodeKind::NodeNull;
        if boolify::node_to_bool(*node[*index].left_node.clone().unwrap()) {
            if cond_status.judge_cond(block_status[1]) {
                cond_status.cond_status_true();
                block_status[0] = node[1 + *index].block.unwrap();
            }
        }
    }
}

unsafe fn node_for(
        node: &Vec<node::CalculNode>,
        index: &usize,
        block_status: &mut [usize; 2],
        cond_status: &mut flags::handle_flag::ControlSynFlag,
) {
    cond_status.flag = node::NodeKind::NodeNull;
    cond_status.make_new_flag(true, block_status[1], node::NodeKind::NodeFor);
    // for文の設定をする
    var_manager().make_new_stack();
    match cond_status.now_loop(
        Some(*index),
        Some(*node[*index].left_node.clone().unwrap())
    ) {
        Ok(_) => {/* そもそもここで、Okが帰ってくることはない */},
        Err(log) => {
            if log != control_syn::ControlSynErr::SETTING {
                log::output_log_l0(log);
            } else {
                block_status[0] = node[1 + *index].block.unwrap();
            }
        }
    }
}

/// インタプリを実行するパイプライン
pub(super) fn run_func(func_process: func::FuncNode) -> type_info::VarValue {
    global_state::var_manager().make_new_stack();
    let mut cond_status = flags::handle_flag::ControlSynFlag::new();
    let mut index: usize = 0;
    // 0は実行可能なブロック、1は現在のブロック
    let mut block_status: [usize; 2] = [1, 0];

    while func_process.nodes.len() >= index {
        // 配列が最後の場所になったら、条件分岐や反復処理のどの制御構文がないか確認
        if func_process.nodes.len() == index {
            crate::update_array_index!(index, cond_status);
        }

        block_status[1] = func_process.nodes[index].block.unwrap();

        if block_status[1] == block_status[0] {
            let result = eval::node_run(func_process.nodes[index].clone());

            match result {
                type_info::VarValue::Flag(syntax_flag) => {
                    cond_status.flag = syntax_flag.clone();

                    match syntax_flag {
                        node::NodeKind::NodeIf => unsafe {cond_branch_flag::node_if(&func_process.nodes, &index, &mut block_status, &mut cond_status);},
                        node::NodeKind::NodeIfElse => unsafe {cond_branch_flag::node_if_else(&func_process.nodes, &index, &mut block_status, &mut cond_status);},
                        node::NodeKind::NodeElse => {
                            cond_status.flag = node::NodeKind::NodeNull;
                            if cond_status.judge_cond(block_status[1]) {
                                block_status[0] = func_process.nodes[1 + index].block.unwrap();
                            }
                        }
                        node::NodeKind::NodeFor => unsafe {node_for(&func_process.nodes, &index, &mut block_status, &mut cond_status);},
                        node::NodeKind::NodeRet => {
                            global_state::var_manager().remove_stack();
                            cond_status.flag = node::NodeKind::NodeNull;
                            return eval::node_run(*func_process.nodes[index].left_node.clone().unwrap());
                        }
                        _ => panic!("panic syntax flag"),
                    }
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
        } else if block_status[1] < block_status[0] {
            if let Some(flag) = cond_status.get_now_flag() {
                match flag {
                    node::NodeKind::NodeIf => {
                        crate::branch_if_cond_flag!(
                            func_process.nodes[index].node_type.clone(),
                            cond_status,
                            block_status
                        );
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
