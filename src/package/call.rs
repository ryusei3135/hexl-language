use std::ffi::CString;
use std::ffi::CStr;

use crate::package::abi;


pub unsafe fn call_function(
    api: *const abi::PluginApi,
    func_name: &str,
    arg: &str,
) -> Option<i32> {
    let api = &*api;

    let entries = std::slice::from_raw_parts(
        api.entries,
        api.entry_count as usize,
    );

    for e in entries {
        let name = CStr::from_ptr(e.name).to_str().ok()?;

        if name == func_name {
            let c_arg = CString::new(arg).ok()?;
            return Some((e.func)(c_arg.as_ptr()));
        }
    }

    None
}