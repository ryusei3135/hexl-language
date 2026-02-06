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
