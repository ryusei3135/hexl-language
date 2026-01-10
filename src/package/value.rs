use std::ffi::c_char;


#[repr(C)]
pub enum ArgType {
    Int,
    Str,
    Bool,
    Void,
}

#[repr(C)]
pub union CValue {
    pub int_value: i32,
    pub str_value: *mut c_char,
    pub bool_value: bool,
}

#[repr(C)]
pub struct VmArgsValue {
    pub arg_type: ArgType,
    pub value: CValue,
}
