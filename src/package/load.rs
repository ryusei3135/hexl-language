use libloading::{Library, Symbol};


pub fn load_func() {
    unsafe {
        let lib = Library::new("./libhello.so").unwrap();

        let hello: Symbol<unsafe extern "C" fn()> =
            lib.get(b"hello").unwrap();

        hello();
    }
}