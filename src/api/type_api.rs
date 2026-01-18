use crate::manager::type_info;
use crate::create_type_api;

create_type_api!(
    Int32: i32,
    Str: String,
    Bool: bool,
    Receiver: String,
    None: bool,
);
