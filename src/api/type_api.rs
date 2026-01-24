use super::*;


create_type_api!(
    Int32: i32,
    Str: String,
    Bool: bool,
    Receiver: String,
    None: bool,
    Array: Vec<Box<VarValue>>,
);


///  反復処理の条件をbooleanで返す
pub fn is_not_zero(value: type_info::VarValue) -> Option<bool> {
    match value {
        type_info::VarValue::Int32(result) => Some(result > 0),
        type_info::VarValue::Str(result) => Some(result.chars().count() != 0),
        _ => None,
    }
}
/// 反復処理でfor文で回す値をデクリメントし、その結果を返す
pub fn dec_and_get_item(
        mut value: type_info::VarValue
) -> Result<type_info::VarValue, control_syn::ControlSynErr> {
    match value {
        type_info::VarValue::Int32(mut result) => {
            result -= 1;
            Ok(type_info::VarValue::Int32(result))
        }
        type_info::VarValue::Str(mut result) => {
            let first = result.chars().next().unwrap();
            type_info::VarValue::Str(result.remove(0).to_string());
            Ok(type_info::VarValue::Str(first.to_string()))
        }
        /// 繰り返す値の型が無効な型
        _ => {
            eprintln!("[err]: value is of invalid type");
            Err(control_syn::ControlSynErr::VALUE_IS_OF_INVALID_TYPE)
        }
    }
}
