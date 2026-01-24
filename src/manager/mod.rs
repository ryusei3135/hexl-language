pub mod func;
pub mod variable;
pub mod global_state;
pub mod type_info;


use std::sync::{OnceLock, Mutex, MutexGuard};
use crate::parse::node;


///  変数の型の情報を持つ列挙型を作成する
#[macro_export]
macro_rules! create_var_type_data {
    ($($member:ident : $type:ty,)*) => {
        ///  変数の値を持つデータつき列挙型
        #[derive(Clone)]
        pub enum VarValue {
            $($member($type)),*
        }
        //  この列挙型は、変数の型情報のみを表す。
        // pub enum VarType {
        //     $($member),*
        // }
    };
}

pub enum VarRegion {
    Stack,
}
