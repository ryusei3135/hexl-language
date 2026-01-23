pub mod variable_api;
pub mod arg_api;
pub mod type_api;


use crate::manager::{variable, global_state, type_info};
use crate::parse::node;
use crate::runner::run;
use crate::manager::func;

// type_api.rs
#[macro_export]
macro_rules! create_type_api {
    ($($member:ident : $type:ty,)*) => {
        //
    };
}
