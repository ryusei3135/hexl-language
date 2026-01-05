#[derive(Debug, Clone)]
pub enum VmValue {
    Int(i64),
    Str(String),
    Bool(bool),
    Void,
    Args(Vec<VmValue>),
}

#[repr(C)]
pub struct CValue {
    type_: i32,
    data: CValueData,
}

#[repr(C)]
pub union CValueData {
    i: i32,
    s: *const i8,
    b: i32,
}

pub type CFunc = unsafe extern "C" fn(*const CValue, usize) -> CValue;
pub type VmFn = Box<
    dyn Fn(&[VmValue]) -> VmValue
    + Send
    + Sync
    + 'static
>;
