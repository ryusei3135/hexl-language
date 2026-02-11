pub mod variable_api;
pub mod arg_api;
pub mod type_api;
pub mod lang_api_type;

use lang_api_type::*;
use crate::manager::*;
use crate::parse::node;
use crate::manager::func;
use crate::manager::VarRegion;
use crate::error::*;
use crate::create_type_api;
use crate::error::control_syn::ControlSynErr;
use crate::runner::control_info::ControlSemantics;
use crate::runner::eval;
use crate::runner::run;

// type_api.rs
#[macro_export]
macro_rules! create_type_api {
    ($($member:ident : $type:ty : $txt:expr,)*) => {
        /// 文字列の型を型の情報に変換
        pub fn change_txt_type_to_type(txt_type: &String) -> type_info::VarType {
            match txt_type.as_str() {
                $($txt => type_info::VarType::$member,)*
                _ => panic!("not found txt type -> {}", txt_type),
            }
        }
        /// 文字列の型と、値としての型を比較し同じ型に分類されるならtrueを返す
        pub fn match_txt_to_value_type(txt: &str, value: &type_info::VarValue) -> bool {
            match (txt, value) {
                $(($txt, type_info::VarValue::$member(_)) => true,)*
                _ => false,
            }
        }
        /// 値としての型を文字列に変換
        pub fn change_var_value_to_txt(value: &type_info::VarValue) -> Result<String, ()> {
            match *value {
                $(type_info::VarValue::$member(_) => Ok($txt.to_string()),)*
                _ => Err(())
            }
        }

        pub fn match_type_kind(
                type_name: &type_info::VarType,
                ret_value: &type_info::VarValue,
        ) -> bool {
            match (type_name, ret_value) {
                $((type_info::VarType::$member, type_info::VarValue::$member(_)) => true,)*
                _ => false,
            }
        }
    };
}
