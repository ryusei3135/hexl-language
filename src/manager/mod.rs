pub mod func;
pub mod variable;
pub mod global_state;
pub mod type_info;


use std::sync::{OnceLock, Mutex, MutexGuard};
use crate::parse::node;
