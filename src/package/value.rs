use std::ffi::c_char;


#[macro_export]  macro_rules! VmValueStruct {
    ($($name:ident : $ty:ty, $field:ident),* $(,)?) => {
        #[repr(C)]
        pub enum ArgType {
            $(
                $name,
            )*
            Void,
        }

        #[repr(C)]
        pub union CValue {
            $(
                pub $field: $ty,
            )*
        }
    };
}

VmValueStruct!(
    Int: i32, int_value,
    Str: *mut c_char, str_value,
    Bool: bool, bool_value,
);

#[repr(C)]
pub struct VmArgsValue {
    pub arg_type: ArgType,
    pub value: CValue,
}
