use std::ffi::c_char;

#[repr(C)]
pub enum ArgType {
    Int32,
    Float32,

    Bool,
    Void,
    Str,
}
#[repr(C)]
pub union CValue {
    pub i32_value: i32,
    pub f32_value: f32,

    pub str_value: *mut c_char,
    pub bool_value: bool,
    pub void_value: u8,
}

#[repr(C)]
pub struct VmArgsValue {
    pub arg_type: ArgType,
    pub value: CValue,
}
