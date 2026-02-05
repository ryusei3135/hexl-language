//! 実行中に制御構文が来た際に使うフラグを保持
use super::*;
use std::sync::{OnceLock, Mutex};

///  条件分岐などの現在の式が何かを代入
static CONTROL_SYN_FLAG: OnceLock<Mutex<node::NodeKind>> = OnceLock::new();


pub fn control_syn_flag(kind: node::NodeKind) {
    *CONTROL_SYN_FLAG
        .get_or_init(|| Mutex::new(node::NodeKind::NodeNull))
        .lock()
        .unwrap() = kind;
}

pub fn get_control_syn_flag() -> node::NodeKind {
    CONTROL_SYN_FLAG
        .get_or_init(|| Mutex::new(node::NodeKind::NodeNull))
        .lock()
        .unwrap()
        .clone()
}
