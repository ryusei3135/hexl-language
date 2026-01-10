use std::collections::HashMap;
use libloading::Library;
use std::ffi::c_char;
use crate::package::value::{ArgType, CValue, VmArgsValue};


pub type NativeFunc = unsafe extern "C" fn(*mut VmArgsValue, usize) -> VmArgsValue;


pub struct NativeFuncData {
    pub lib: Library,
    pub func: HashMap<String, NativeFunc>,
}
