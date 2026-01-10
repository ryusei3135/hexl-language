use std::collections::HashMap;
use libloading::Library;
use crate::package::value::VmArgsValue;


pub type NativeFunc = unsafe extern "C" fn(*mut VmArgsValue, usize) -> VmArgsValue;


pub struct NativeFuncData {
    pub lib: Library,
    pub func: HashMap<String, NativeFunc>,
}
