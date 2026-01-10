// use std::collections::HashMap;
// use libloading::{Library, Symbol};
// use crate::package::abi;




// fn call_native(func: NativeFn, args: Vec<Value>) -> Value {
//     let mut natives = HashMap::new();

//     let lib = unsafe { Library::new("libplugin.so")? };
//     let add: Symbol<NativeFn> = unsafe { lib.get(b"add")? };

//     natives.insert(
//         "add".to_string(),
//         NativeFunction { _lib: lib, func: add },
//     );

//     match args.as_slice() {
//         [Value::Int(a), Value::Int(b)] => {
//             let result = unsafe { func(*a, *b) };
//             Value::Int(result)
//         }
//         _ => panic!("invalid arguments"),
//     }

//     let f = natives.get("add").unwrap();
//     let result = unsafe { (f.func)(1, 2) };
// }
