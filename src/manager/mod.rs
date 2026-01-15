pub mod func;
pub mod variable;
pub mod global_state;


use std::sync::{OnceLock, Mutex, MutexGuard};
use crate::parse::node;
