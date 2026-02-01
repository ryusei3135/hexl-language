pub mod variable_api;
pub mod arg_api;
pub mod type_api;
pub mod lang_api_type;

use lang_api_type::*;
use crate::manager::*;
use crate::parse::node;
use crate::runner::run;
use crate::manager::func;
use crate::manager::VarRegion;
use crate::error::*;
use crate::create_type_api;
use crate::error::control_syn::ControlSynErr;
use crate::runner::control_info::ControlSemantics;
use crate::runner::run::node_run;

// type_api.rs
#[macro_export]
macro_rules! create_type_api {
    ($($member:ident : $type:ty : $txt:expr,)*) => {
        pub fn change_txt_type_to_type(txt_type: String) -> type_info::VarType {
            match txt_type.as_str() {
                $($txt => type_info::VarType::$member,)*
                _ => panic!("not found txt type"),
            }
        }
    };
}

// mod iter {
//     use super::*
//     // pub fn update_loop_var(
//     //         iterable_value: type_info::VarValue,
//     //         now_value: &Option<type_info::VarValue>,
//     //         binds_var: bool,
//     //         loop_cond: &node::CalculNode,
//     // ) -> IterStatus {
//     //     match (iterable_value, now_value.clone().unwrap()) {
//     //         (type_info::VarValue::Int32(l), type_info::VarValue::Int32(r)) => {
//     //             if l != r {
//     //                 Ok(
//     //                     (
//     //                         true,
//     //                         type_info::VarValue::Int32(r + 1),
//     //                         if binds_var {
//     //                             ControlSemantics::BindsVar(loop_cond.value.clone())
//     //                         } else {
//     //                             ControlSemantics::NotBinds
//     //                         }
//     //                     )
//     //                 )
//     //             } else {
//     //                 Ok((false, type_info::VarValue::Int32(r + 1), ControlSemantics::End))
//     //             }
//     //         }
//     //         _ => Err(ControlSynErr::ValueIsOfInvalidType),
//     //     }
//     // }
//     /// for文の情報を更新
//     /// # 引数
//     /// - iter_now_status =
//     ///     現在のfor文の情報

// }
