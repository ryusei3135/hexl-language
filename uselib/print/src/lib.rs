use std::ffi::c_char;

unsafe extern "C" {
    pub fn cprintln(fmt: *const c_char, ...);
}

#[macro_export]
macro_rules! cprint {
    ($fmt:literal $(, $arg:expr)* $(,)?) => {{
        let fmt = concat!($fmt, "\0");

        unsafe {
            $crate::cprintln(
                fmt.as_ptr() as *const std::ffi::c_char
                $(, $arg)*
            );
        }
    }};
}