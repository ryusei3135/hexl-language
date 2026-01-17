use super::*;


///  変数の型の情報を持つ列挙型を作成する
macro_rules! create_var_type_data {
    ($($member:ident : $type:ty,)*) => {
        ///  変数の値を持つデータつき列挙型
        pub enum VarValue {
            $($member($type)),*
        }
        ///  この列挙型は、変数の型情報のみを表す。
        pub enum VarType {
            $($member),*
        }
    };
}

/// 変数の型の情報に関する列挙型を展開
create_var_type_data!(
    Int32: i32,
    Str: String,
    Bool: bool,
);
