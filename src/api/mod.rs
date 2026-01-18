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
        ///  VarValue同士を比較し、計算可能ならばtrueを返す
        pub fn match_type(left: type_info::VarValue, right: type_info::VarValue) -> bool {
            match (left, right) {
                $((type_info::VarValue::$member(_), type_info::VarValue::$member(_)) => true,)*
                _ => false,
            }
        }
        ///  渡された値の型のタイプを返す
        pub fn get_value_type(value: type_info::VarValue) -> type_info::VarType {
            match value {
                $(type_info::VarValue::$member(_) => type_info::VarType::$member,)*
                _ => panic!(""),
            }
        }
    };
}
