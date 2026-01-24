use super::*;


pub fn get_variable_info(name: String) -> variable::VariableInfo {
    global_state::var_manager().get_var(name)
}

pub fn update_variable_value(name: String, new_value: type_info::VarValue) {
    if !global_state::var_manager().update_var(name, new_value) {
        std::process::exit(1);
    }
}

pub fn call_var_value(name: String) -> type_info::VarValue {
    let var_info = get_variable_info(name);
    var_info.value
}

pub fn define_var(node: node::CalculNode) -> type_info::VarValue {
    let var_name = node.value;

    let value = run::node_run(*node.right_node.clone().unwrap());

    global_state::var_manager().add_var(
        var_name.clone(),
        value,
    );
    return call_var_value(var_name.clone());
}

//  変数の値の上書き
pub fn update_var_value(
        node: node::CalculNode
) -> Result<type_info::VarValue, define_msg::VarErrorOrLog> {
    if node.left_node.clone().unwrap().node_type == node::NodeKind::NodeVarName {
        let var_name = node.left_node.unwrap().value.clone();
        let value = run::node_run(*node.right_node.unwrap());

        global_state::var_manager().update_var(
            var_name.clone(),
            value,
        );
        return Ok(call_var_value(var_name.clone()));
    } else {
        println!("syntax err: assign var");
    }
    return Err(define_msg::VarErrorOrLog::VarIsNotDefined);
}


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
        value: type_info::VarValue
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
        // 繰り返す値の型が無効な型
        _ => {
            eprintln!("[err]: value is of invalid type");
            Err(control_syn::ControlSynErr::ValueIsOfInvalidType)
        }
    }
}
