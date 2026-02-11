pub mod func;
pub mod variable;
pub mod type_info;


use crate::parse::*;
use crate::error::define_msg;
use crate::create_var_type_data;
use crate::api::type_api;

///  変数の型の情報を持つ列挙型を作成する
#[macro_export]
macro_rules! create_var_type_data {
    ($($member:ident : $type:ty,)*) => {
        ///  変数の値を持つデータつき列挙型
        #[derive(Clone, PartialEq, Debug)]
        pub enum VarValue {
            Flag(node::NodeKind),
            $($member($type)),*
        }
        ///  この列挙型は、変数の型情報のみを表す。
        #[derive(Clone, PartialEq)]
        pub enum VarType {
            $($member),*
        }
    };
}

/// 変数の領域を表す列挙型
#[derive(Clone, PartialEq)]
pub enum VarRegion {
    Stack,
    Heap,
    Static,
}
