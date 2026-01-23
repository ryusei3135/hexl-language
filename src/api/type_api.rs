use crate::manager::type_info;
use crate::create_type_api;

create_type_api!(
    Int32: i32,
    Str: String,
    Bool: bool,
    Receiver: String,
    None: bool,
    Array: Vec<Box<VarValue>>,
);


///  反復処理で使う 反復処理の条件をbooleanで返す
pub fn is_not_zero(value: type_info::VarValue) -> Option<bool> {
    match value {
        type_info::VarValue::Int32(result) => Some(result > 0),
        type_info::VarValue::Str(result) => Some(result.chars().count() != 0),
        _ => None,
    }
}
