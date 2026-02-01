pub mod control_info;
pub mod run;
pub mod flags;
mod boolify;
mod expand;
mod result;


use std::sync::{OnceLock, Mutex};
use crate::manager::{
    func,
    variable,
    type_info,
    VarRegion,
    global_state::*,
};
use crate::package::manage;
use crate::api::{
    lang_api_type::*,
    variable_api::*,
    variable_api,
    arg_api,
};
use crate::parse::node;
use crate::runner::result::output_log;
use crate::error::control_syn;
