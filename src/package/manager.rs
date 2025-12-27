use libloading::Library;

use crate::package;


struct LibData {
    pub name: String,
    pub data: Library,
}

pub struct PackageManager {
    pub libs: Vec<LibData>
}

impl PackageManager {
    pub fn new() -> Self {
        Self {
            libs: Vec::<LibData>::new(),
        }
    }

    pub fn load_lib(&mut self, name: String) {
        unsafe {
            let lib = Library::new(format!("./{}", name)).unwrap();
            self.libs.push(
                LibData {
                    name: name,
                    data: lib,
                }
            );

            let api = package::load::load_api(&self.libs.last().unwrap().data);

            package::load::dump_entries(api);

            let result = package::call::call_function(api, "print", "hello world!!");
        }
    }
}