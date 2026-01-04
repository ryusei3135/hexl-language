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


pub struct CondStatus {
    pub status: Vec<(bool, i32)>,
}

impl CondStatus {
    pub fn new() -> Self {
        Self {
            status: Vec::<(bool, i32)>::new()
        }
    }

    pub fn push(&mut self, cond: bool, area: i32) {
        if self.status.len() > 0 {
            if self.status.last().unwrap().1 == area {
                if let Some((flag, value)) = self.status.last_mut() {
                    *flag = cond;
                    return;
                }
            }
        }

        self.status.push((cond, area));
    }

    pub fn judge_cond(&self, now_area: i32) -> bool {
        //  エリアが同じかつ、今までの条件で、trueになってなければ、今の条件を実行可能
        self.status.last().unwrap().1 == now_area && !self.status.last().unwrap().0
    }

    pub fn cond_true(&mut self) {
        if let Some((flag, value)) = self.status.last_mut() {
            *flag = true;   // ← list[last].0 を変更
        }
    }

    pub fn del(&mut self) {
        self.status.pop().unwrap();
    }
}
