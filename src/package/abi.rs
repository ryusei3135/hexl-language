use std::collections::HashMap;
use libloading::Library;
// use crate::package::value::VmArgsValue;
use crate::lib::*;


pub type NativeFunc = unsafe extern "C" fn(args: *mut VmArgsValue, len: usize) -> VmArgsValue;


pub struct NativeFuncData {
    pub lib: Library,
    pub func: HashMap<String, NativeFunc>,
}
