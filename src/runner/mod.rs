pub mod run;
pub mod runtime;
mod boolify;
mod expand;
mod result;


use std::sync::{OnceLock, Mutex};
use crate::manager::{
    func,
    variable,
    type_info,
    global_state::*,
};
use crate::package::manage;
use crate::api::{
    variable_api::*,
    variable_api,
    arg_api,
};
use crate::parse::node;
use crate::error::control_syn;
