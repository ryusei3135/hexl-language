use super::*;
use crate::runner::control_info::ControlSemantics;
use crate::runner::run::node_run;


pub fn get_variable_info(name: String) -> Result<variable::VariableInfo, define_msg::VarErrorOrLog> {
    global_state::var_manager().get_var(name)
}

pub fn update_variable_value(name: String, new_value: type_info::VarValue) {
    if !global_state::var_manager().update_var(name, new_value) {
        std::process::exit(1);
    }
}

pub fn call_var_value(name: String) -> type_info::VarValue {
    match get_variable_info(name) {
        Ok(var_info) => var_info.value,
        Err(_) => panic!("variable is not defined"),
    }
}

pub fn define_var(node: node::CalculNode) -> type_info::VarValue {
    let var_name = node.value;

    let value = run::node_run(*node.right_node.clone().unwrap());

    global_state::var_manager().add_var(
        var_name.clone(),
        value,
        VarRegion::Stack,
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


pub fn is_for_iterable(
        loop_cond: node::CalculNode,
        now_value: &Option<type_info::VarValue>
) -> Option<(bool, type_info::VarValue, ControlSemantics)> {
    if loop_cond.node_type == node::NodeKind::NodeIn {
        let iterable_value = node_run(*loop_cond.left_node.clone().unwrap());
        let binds_var = {
            if loop_cond.node_type == node::NodeKind::NodeIn {
                true
            } else {
                false
            }
        };

        if *now_value == None {
            return match iterable_value {
                type_info::VarValue::Int32(_) => {
                    Some(
                        (
                            true,
                            type_info::VarValue::Int32(0),
                            if binds_var {
                                ControlSemantics::binds_var(loop_cond.value.clone())
                            } else {
                                ControlSemantics::not_binds
                            }
                        )
                    )
                }
                _ => {
                    None
                }
            };
        }

        match (iterable_value, now_value.clone().unwrap()) {
            (type_info::VarValue::Int32(l), type_info::VarValue::Int32(r)) => {
                if l != r {
                    return Some(
                        (
                            true,
                            type_info::VarValue::Int32(r + 1),
                            ControlSemantics::binds_var(loop_cond.value.clone()),
                        )
                    );
                } else {
                    return Some((false, type_info::VarValue::Int32(r + 1), ControlSemantics::End));
                }
            }
            _ => return None,
        }
    }

    None
}
