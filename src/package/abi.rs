use std::os::raw::c_char;



#[repr(C)]
pub struct PluginEntry {
    pub name: *const c_char,
    pub func: unsafe extern "C" fn(*const c_char) -> i32,
}

#[repr(C)]
pub struct PluginApi {
    pub api_version: u32,
    pub entries: *const PluginEntry,
    pub entry_count: u32,
}