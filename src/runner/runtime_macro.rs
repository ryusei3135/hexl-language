/// 処理中に条件分岐の制御構文が来たときに
/// 現在の立っているフラグや現在のノードの種類
/// などを比較し処理を分岐させる
#[macro_export]
macro_rules! branch_if_cond_flag {
    ($flag:expr, $cond_status:expr, $block:ident) => {
        match flags::flag_switch::set_runtime_flag(&$flag) {
            Ok(syn) => {
                // ノードの種類が条件分岐でないなら
                // 条件分岐の情報を削除
                match syn {
                    flags::flag_switch::SynFlagKind::For => {
                        $cond_status.del();
                    }
                    flags::flag_switch::SynFlagKind::Cond => {}
                }
            }
            // ノードの種類が条件分岐でないので条件分岐のデータを削除
            Err(_) => {
                $cond_status.del();
            }
        }
        $block[0] = $block[1];
        continue;
    };
}

#[macro_export]
macro_rules! update_array_index {
    ($index:ident, $cond_status:ident) => {
        var_manager().remove_stack();
        var_manager().make_new_stack();

        match $cond_status.now_loop(None, None) {
            Ok(cond_location) => {
                $index = cond_location + 1;
                continue;
            }
            Err(log) => {
                $cond_status.del();
                var_manager().remove_stack();
                log::output_log_l0(log);
                break;
            }
        }
    };
}
