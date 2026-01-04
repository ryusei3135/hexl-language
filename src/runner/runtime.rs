use crate::manager::{func, variable};
use std::sync::{OnceLock, Mutex, MutexGuard};

use crate::parse::node;


static PROCESS_KIND: OnceLock<Mutex<node::NodeKind>> = OnceLock::new();


pub fn process_kind(kind: node::NodeKind) {
    *PROCESS_KIND
        .get_or_init(|| Mutex::new(node::NodeKind::NodeNull))
        .lock()
        .unwrap() = kind;
}

pub fn get_process_kind() -> node::NodeKind {
    PROCESS_KIND
        .get_or_init(|| Mutex::new(node::NodeKind::NodeNull))
        .lock()
        .unwrap()
        .clone()
}
