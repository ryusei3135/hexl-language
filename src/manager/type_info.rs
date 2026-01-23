use crate::create_var_type_data;

create_var_type_data!(
    Int32: i32,
    Str: String,
    Bool: bool,
    Receiver: String,
    None: bool,
    Array: Vec<Box<VarValue>>,
);
