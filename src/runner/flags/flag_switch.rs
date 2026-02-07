use super::*;

/// set_runtime_flagでどの制御構文
/// のフラグが立ったのかを表現する
pub enum SynFlagKind {
    Cond,
    For,
}

/// 制御構文のノードを探しフラグを立てる
pub fn set_runtime_flag(
        flag: &node::NodeKind,
) -> Result<SynFlagKind, node::NodeKind> {
    match *flag {
        node::NodeKind::NodeIf => {
            Ok(SynFlagKind::Cond)
        }
        node::NodeKind::NodeIfElse => {
            Ok(SynFlagKind::Cond)
        }
        node::NodeKind::NodeElse => {
            Ok(SynFlagKind::Cond)
        }
        node::NodeKind::NodeFor => {
            Ok(SynFlagKind::For)
        }
        _ => Err(flag.clone())
    }
}

/// 制御構文などで、構文の中のスタック領域を操作
/// もし条件分岐で、条件が最後の条件ならスタックを
/// 削除,まだ続くならフラグを消す
/// # 引数
/// - result
///     現在の一行の実行結果
/// - cond_status
///     実行中のフラグの情報
pub fn handle_del_stack_flag(
        result: &type_info::VarValue,
        cond_status: &mut flags::handle_flag::ControlSynFlag,
) -> bool {
    // フラグでないならスタックを削除
    if let type_info::VarValue::Flag(syntax_flag) = result {
        // フラグが条件分岐であることを確認
        // 条件分岐ならスタックを削除する
        // フラグを削除
        match flags::flag_switch::set_runtime_flag(&syntax_flag) {
            Ok(flags::flag_switch::SynFlagKind::Cond) => false,
            _ => {
                cond_status.del();
                false
            }
        }
    } else {
        cond_status.del();
        false
    }
}
