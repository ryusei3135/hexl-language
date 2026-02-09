use super::*;

create_var_type_data!(
    Int32: i32,
    Float32: f32,
    Str: String,
    Bool: bool,
    Receiver: String,
    Null: bool,
    Array: Vec<Box<VarValue>>,
);
