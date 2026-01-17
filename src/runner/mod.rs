pub mod run;
pub mod runtime;


use std::sync::{OnceLock, Mutex};
use crate::parse::node;
use crate::manager::global_state::{func_manager, var_manager};
use crate::manager::variable;
use crate::manager::func;
use crate::package::manage;
use crate::api::{variable_api, arg_api};
