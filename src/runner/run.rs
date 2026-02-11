//! インタプリタのパイプライン
use super::*;

/// 条件分岐のフラグを立てるかどうかを制御
mod cond_branch_flag {
    use super::*;

    pub unsafe fn node_if(
            runtime: &mut Runtime,
            cond_flags: &mut flags::handle_flag::ControlSynFlag,
            node: &Vec<node::CalculNode>,
            index: &usize,
            block_status: &mut flags::BlockFlag,
    ) -> bool {
        if boolify::node_to_bool(runtime, *node[*index].left_node.clone().unwrap()) {
            //  trueを代入すると、つながっている条件分岐がスキップされる
            cond_flags.make_new_flag(true, block_status.now, node::NodeKind::NodeIf);
            block_status.execute = node[1 + *index].block.unwrap();
            false
        } else {
            cond_flags.make_new_flag(false, block_status.now, node::NodeKind::NodeIf);
            true
        }
    }

    pub unsafe fn node_if_else(
            runtime: &mut Runtime,
            cond_flags: &mut flags::handle_flag::ControlSynFlag,
            node: &Vec<node::CalculNode>,
            index: &usize,
            block_status: &mut flags::BlockFlag,
    ) -> bool {
        if boolify::node_to_bool(runtime, *node[*index].left_node.clone().unwrap()) {
            return if cond_flags.judge_cond(block_status.now.clone()) {
                cond_flags.cond_status_true();
                block_status.execute = node[1 + *index].block.unwrap();
                false
            } else {
                true
            };
        }
        true
    }
}

/// for文のフラグを立て、for文の情報を初期化
unsafe fn node_for(
        runtime: &mut Runtime,
        cond_flags: &mut flags::handle_flag::ControlSynFlag,
        node: &Vec<node::CalculNode>,
        index: &usize,
        block_status: &mut flags::BlockFlag,
) {
    cond_flags.make_new_flag(true, block_status.now.clone(), node::NodeKind::NodeFor);
    // for文の設定をする
    runtime.all_info.var_info.make_new_stack();
    match cond_flags.now_loop(
        runtime,
        Some(*index),
        Some(*node[*index].left_node.clone().unwrap())
    ) {
        Ok(_) => {/* そもそもここで、Okが帰ってくることはない */},
        Err(log) => {
            if log != control_syn::ControlSynErr::SETTING {
                log::output_log_l0(log);
            } else {
                block_status.execute = node[1 + *index].block.unwrap();
            }
        }
    }
}

pub struct Runtime {
    pub all_info: node::AllInfo,
}

/// インタプリタを実行する
impl Runtime {
    pub fn new(all_info: node::AllInfo) -> Self {
        Self {
            all_info: all_info,
        }
    }
    /// エントリーポイントを呼び出す
    pub fn start_process(&mut self) {
        let start_process = self.all_info.func_info.get_func("start");

        self.run_func(start_process, &None);
    }

    /// インタプリを実行するパイプライン
    pub fn run_func(
            &mut self,
            func_process: func::FuncNode,
            args_value: &Option<node::CalculNode>
    ) -> type_info::VarValue {
        let mut cond_flags = handle_flag::ControlSynFlag::new();

        self.all_info.var_info.make_scope();
        self.all_info.var_info.make_new_stack();
        if let Some(args) = args_value {
            arg_api::make_args_var(self, &func_process.args, args);
        }

        let mut index: usize = 0;
        // 0は実行可能なブロック、1は現在のブロック
        let mut block_status = flags::BlockFlag {
            execute: 1,
            now: 0,
        };
        let mut del_stack: bool = false;

        while func_process.nodes.len() >= index {
            // 配列が最後の場所になったら、条件分岐や反復処理のどの制御構文のフラグが
            // 立っていないかを確認
            if func_process.nodes.len() == index {
                if let Some(flag) = cond_flags.get_now_flag() {
                    match flag {
                        node::NodeKind::NodeIf => {
                            cond_flags.del();
                            continue;
                        }
                        node::NodeKind::NodeFor => {
                            crate::update_array_index!(
                                index,
                                block_status,
                                self,
                                cond_flags
                            );
                        }
                        _ => panic!("[err: run func run]"),
                    }
                }
                break;
            }

            block_status.now = func_process.nodes[index].block.unwrap();

            if block_status.now == block_status.execute {
                let result = eval::node_run(self, &None, func_process.nodes[index].clone());

                if del_stack {
                    del_stack = flag_switch::handle_del_stack_flag(&result, &mut cond_flags);
                }

                match result {
                    type_info::VarValue::Flag(syntax_flag) => {
                        match syntax_flag.clone() {
                            node::NodeKind::NodeIf => {
                                unsafe {
                                    del_stack = cond_branch_flag::node_if(
                                        self,
                                        &mut cond_flags,
                                        &func_process.nodes,
                                        &index,
                                        &mut block_status,
                                    );
                                }
                            }
                            node::NodeKind::NodeIfElse => {
                                unsafe {
                                    del_stack = cond_branch_flag::node_if_else(
                                        self,
                                        &mut cond_flags,
                                        &func_process.nodes,
                                        &index,
                                        &mut block_status,
                                    );
                                }
                            }
                            node::NodeKind::NodeElse => {
                                if cond_flags.judge_cond(block_status.now) {
                                    block_status.execute = func_process.nodes[1 + index].block.unwrap();
                                } else {
                                    del_stack = true;
                                }
                            }
                            node::NodeKind::NodeFor => {
                                unsafe {
                                    node_for(
                                        self,
                                        &mut cond_flags,
                                        &func_process.nodes,
                                        &index,
                                        &mut block_status,
                                    );
                                }
                            }
                            node::NodeKind::NodeRet => {
                                let r = eval::node_run(self, &None, *func_process.nodes[index].left_node.clone().unwrap());
                                if type_api::match_type_kind(&func_process.ret_type, &r) {
                                    return {
                                        self.all_info.var_info.remove_stack();
                                        self.all_info.var_info.remove_scope();
                                        r
                                    };
                                } else {
                                    panic!("Return type mismatch");
                                }
                            }
                            _ => panic!("panic syntax flag"),
                        }
                    }
                    _ => {
                        println!("[{}] [value]", match result {
                            type_info::VarValue::Int32(v) => v.to_string(),
                            type_info::VarValue::Bool(v) => v.to_string(),
                            type_info::VarValue::Float32(v) => v.to_string(),
                            type_info::VarValue::Str(v) => v,
                            _ => {
                                index += 1;
                                continue;
                            },
                        });
                    }
                }
            } else if block_status.now < block_status.execute {
                if let Some(flag) = cond_flags.get_now_flag() {
                    match flag {
                        node::NodeKind::NodeIf => {
                            crate::branch_if_cond_flag!(
                                func_process.nodes[index].node_type.clone(),
                                cond_flags,
                                block_status
                            );
                        }
                        node::NodeKind::NodeFor => {
                            crate::update_array_index!(
                                index,
                                block_status,
                                self,
                                cond_flags
                            );

                            block_status.execute = block_status.now;
                            continue;
                        }
                        _ => panic!("[err: run func run]"),
                    }
                } else {
                    eprintln!("[what?]44");
                }
            }
            index += 1;
        }
        self.all_info.var_info.remove_stack();
        self.all_info.var_info.remove_scope();
        type_info::VarValue::Null(false)
    }
}
