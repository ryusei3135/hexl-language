//! このフォルダで使うマクロを記述

/// 処理中に条件分岐の制御構文が来たときに
/// 現在の立っているフラグや現在のノードの種類
/// などを比較し処理を分岐させる
#[macro_export]
macro_rules! branch_if_cond_flag {
    ($flag:expr, $cond_status:expr, $block:ident) => {
        match flag_switch::set_runtime_flag(&$flag) {
            Ok(syn) => {
                // ノードの種類が条件分岐でないなら
                // 条件分岐の情報を削除
                match syn {
                    flag_switch::SynFlagKind::For => {
                        $cond_status.del();
                    }
                    flag_switch::SynFlagKind::Cond => {}
                }
            }
            // ノードの種類が条件分岐でないので条件分岐のデータを削除
            Err(_) => {
                $cond_status.del();
            }
        }
        $block.execute = $block.now;
        continue;
    };
}

#[macro_export]
macro_rules! update_array_index {
    ($index:ident, $block:expr, $runtime:expr, $cond_flags:expr) => {
        $runtime.all_info.var_info.remove_stack();
        $runtime.all_info.var_info.make_new_stack();

        match $cond_flags.now_loop($runtime, None, None) {
            Ok(cond_location) => {
                // ブロックをfor文の最初に戻す
                $block.execute = $cond_flags.get_me_block().unwrap() + 1;
                $index = cond_location + 1;
                continue;
            }
            Err(log) => {
                $cond_flags.del();
                $runtime.all_info.var_info.remove_stack();
                if log != control_syn::ControlSynErr::EndLoop {
                    log::output_log_l0(log);
                    break;
                }
            }
        }
    };
}

#[macro_export]
macro_rules! calcul_by_type {
    (+, $bind:expr, $runtime:ident, $node:path, $($type:path),*) => {
        match (get_left_value($runtime, $bind, &$node), get_right_value($runtime, $bind, &$node)) {
            $(($type(l), $type(r)) => $type(l + r),)*
            _ => panic!("node run 2 add"),
        }
    };
    (-, $bind:expr, $runtime:ident, $node:path, $($type:path),*) => {
        match (get_left_value($runtime, $bind, &$node), get_right_value($runtime, $bind, &$node)) {
            $(($type(l), $type(r)) => $type(l - r),)*
            _ => panic!("node run 2 add"),
        }
    };
    (*, $bind:expr, $runtime:ident, $node:path, $($type:path),*) => {
        match (get_left_value($runtime, $bind, &$node), get_right_value($runtime, $bind, &$node)) {
            $(($type(l), $type(r)) => $type(l * r),)*
            _ => panic!("node run 2 add"),
        }
    };
    (/, $bind:expr, $runtime:ident, $node:path, $($type:path),*) => {
        match (get_left_value($runtime, $bind, &$node), get_right_value($runtime, $bind, &$node)) {
            $(($type(l), $type(r)) => $type(l / r),)*
            _ => panic!("node run 2 add"),
        }
    };
    (%, $bind:expr, $runtime:ident, $node:path, $($type:path),*) => {
        match (get_left_value($runtime, $bind, &$node), get_right_value($runtime, $bind, &$node)) {
            $(($type(l), $type(r)) => $type(l % r),)*
            _ => panic!("node run 2 add"),
        }
    };
}

#[macro_export]
macro_rules! comper_op_type {
    (==, $bind:expr, $runtime:ident, $node:path, $($type:path),*) => {
        match (get_left_value($runtime, $bind, &$node), get_right_value($runtime, $bind, &$node)) {
            $(($type(l), $type(r)) => type_info::VarValue::Bool(l == r),)*
            _ => panic!("node run 2 add"),
        }
    };
    (!=, $bind:expr, $runtime:ident, $node:path, $($type:path),*) => {
        match (get_left_value($runtime, $bind, &$node), get_right_value($runtime, $bind, &$node)) {
            $(($type(l), $type(r)) => type_info::VarValue::Bool(l != r),)*
            _ => panic!("node run 2 add"),
        }
    };
    (<=, $bind:expr, $runtime:ident, $node:path, $($type:path),*) => {
        match (get_left_value($runtime, $bind, &$node), get_right_value($runtime, $bind, &$node)) {
            $(($type(l), $type(r)) => type_info::VarValue::Bool(l <= r),)*
            _ => panic!("node run 2 add"),
        }
    };
    (>=, $bind:expr, $runtime:ident, $node:path, $($type:path),*) => {
        match (get_left_value($runtime, $bind, &$node), get_right_value($runtime, $bind, &$node)) {
            $(($type(l), $type(r)) => type_info::VarValue::Bool(l >= r),)*
            _ => panic!("node run 2 add"),
        }
    };
    (<, $bind:expr, $runtime:ident, $node:path, $($type:path),*) => {
        match (get_left_value($runtime, $bind, &$node), get_right_value($runtime, $bind, &$node)) {
            $(($type(l), $type(r)) => type_info::VarValue::Bool(l < r),)*
            _ => panic!("node run 2 add"),
        }
    };
    (>, $bind:expr, $runtime:ident, $node:path, $($type:path),*) => {
        match (get_left_value($runtime, $bind, &$node), get_right_value($runtime, $bind, &$node)) {
            $(($type(l), $type(r)) => type_info::VarValue::Bool(l > r),)*
            _ => panic!("node run 2 add"),
        }
    };
}
