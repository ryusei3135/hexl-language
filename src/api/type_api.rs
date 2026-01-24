use super::*;

create_type_api!(
    Int32: i32,
    Str: String,
    Bool: bool,
    Receiver: String,
    Null: bool,
    Array: Vec<Box<VarValue>>,
);
