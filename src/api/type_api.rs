use super::*;

create_type_api!(
    Int32: i32 : "i32",
    Float32: f32 : "f32",
    Str: String : "str",
    Bool: bool : "bool",
    Receiver: String : "none",
    Null: bool : "null",
    Array: Vec<Box<VarValue>> : "arr",
);
