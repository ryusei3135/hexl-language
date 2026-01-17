use std::collections::HashMap;
use libloading::{Library, Symbol};
use crate::package::{abi, yaml};



pub fn load_exported_functions(
    lib_config: yaml::setting::NativeLibrary,
    lib_dir: String
) -> Result<abi::NativeFuncData, Box<dyn std::error::Error>> {
    let lib = unsafe { Library::new(&(lib_dir.clone() + &lib_config.metadata.filename))? };

    let mut map = HashMap::new();

    unsafe {
        for f in &lib_config.functions {
            let symbol: Symbol<abi::NativeFunc> = lib.get(f.name.as_bytes()).expect("this func is not fund lib");

            // 関数ポインタをコピー（ここが重要）
            let ptr: abi::NativeFunc = *symbol;

            map.insert(
                f.name.clone(),
                abi::NativeCall {
                    func_ptr: ptr,
                    args_type: f.args_types.clone(),
                }
            );
        }
    }

    Ok(abi::NativeFuncData {
        module_name: "*null*".to_string(),
        lib: lib.into(),
        func: map,
    })
}
