//! このモジュールは実行中に発生するフラグの管理や
//! フラグを使い実行結果を制御する

pub mod handle_flag;
pub mod flag_switch;

use crate::runner::*;

#[derive(Debug)]
pub struct BlockFlag {
    pub execute: usize,
    pub now: usize,
}
