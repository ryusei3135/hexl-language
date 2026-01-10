use std::collections::HashMap;
use std::sync::Arc;
use libloading::Library;
// use crate::package::value::VmArgsValue;
use crate::lib::*;


pub type NativeFunc = unsafe extern "C" fn(args: *mut VmArgsValue, len: usize) -> VmArgsValue;

#[derive(Clone)]
pub struct NativeFuncData {
    pub module_name: String,
    pub lib: Arc<Library>,
    pub func: HashMap<String, NativeFunc>,
}
