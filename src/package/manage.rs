use super::*;

///  ネイティブ関数に渡す引数を作成
fn make_vm_args(
        runtime: &mut run::Runtime,
        args: node::CalculNode,
        mut args_type: Vec<String>,
) -> Vec<lib::VmArgsValue> {
    let mut vm_args = Vec::<lib::VmArgsValue>::new();
    let mut now_node = args.clone();
    while now_node.node_type == node::NodeKind::NodeArgsValue {
        let arg_node = *now_node.left_node.clone().unwrap();

        vm_args.push(
            match args_type.remove(0).as_str() {
                "str" => {
                    let c_value = lib::CValue {
                        str_value: match eval::node_run(runtime, arg_node) {
                            type_info::VarValue::Str(r) => CString::new(r).unwrap().into_raw(),
                            _ => panic!("un match type"),
                        }
                    };
                    lib::VmArgsValue {
                        arg_type: lib::ArgType::Str,
                        value: c_value,
                    }
                }
                "int" => {
                    let c_value = lib::CValue {
                        i32_value: match eval::node_run(runtime, arg_node) {
                            type_info::VarValue::Int32(r) => r,
                            _ => panic!("un match type"),
                        }
                    };
                    lib::VmArgsValue {
                        arg_type: lib::ArgType::Int32,
                        value: c_value,
                    }
                }
                "f32" => {
                    let c_value = lib::CValue {
                        f32_value: match eval::node_run(runtime, arg_node) {
                            type_info::VarValue::Float32(r) => r,
                            _ => panic!("un match type"),
                        }
                    };
                    lib::VmArgsValue {
                        arg_type: lib::ArgType::Float32,
                        value: c_value,
                    }
                }
                "bool" => {
                    let c_value = lib::CValue {
                        bool_value: if let Ok(boolean) = arg_node.value.clone().parse::<bool>() {
                            boolean
                        } else {
                            true
                        }
                    };
                    lib::VmArgsValue {
                        arg_type: lib::ArgType::Bool,
                        value: c_value,
                    }
                }
                "void" => {
                    let c_value = lib::CValue {
                        void_value: 0,
                    };
                    lib::VmArgsValue {
                        arg_type: lib::ArgType::Void,
                        value: c_value,
                    }
                }
                _ => panic!("what is arg type"),
            }
        );
        now_node = *now_node.right_node.clone().unwrap();
    }
    return vm_args;
}

fn get_vm_ret_value(ret_value: lib::VmArgsValue) -> Option<type_info::VarValue> {
    unsafe {
        return match ret_value.arg_type {
            lib::ArgType::Str => {
                Some(type_info::VarValue::Str(CStr::from_ptr(ret_value.value.str_value).to_string_lossy().into_owned()))
            }
            lib::ArgType::Int32 => Some(type_info::VarValue::Int32(ret_value.value.i32_value)),
            lib::ArgType::Float32 => Some(type_info::VarValue::Float32(ret_value.value.f32_value)),
            lib::ArgType::Bool => Some(type_info::VarValue::Bool(ret_value.value.bool_value)),
            lib::ArgType::Void => None,
        };
    }
}

pub fn run_native_func(
        runtime: &mut run::Runtime,
        receiver_name: &String,
        func_name: &String,
        args: &node::CalculNode
) -> Option<type_info::VarValue> {
    let func = runtime.all_info.native_info
        .iter()
        .find(|n| n.module_name == *receiver_name)
        .map(|n| n.clone())
        .unwrap_or_else(|| {
            panic!("this func is not found(lib)");
        });
    let func_call_data = func.func
        .get(func_name)
        .expect("this netive func is false");
    let vm_func = func_call_data.func_ptr;
    let ret_value = unsafe {
        let vm_args = make_vm_args(runtime, args.clone(), func_call_data.args_type.clone());
        let ret = vm_func(vm_args.as_ptr() as *mut lib::VmArgsValue, vm_args.len());
        get_vm_ret_value(ret)
    };
    ret_value
}

/// コンパイル済みの関数を追加
/// # 引数
/// - native_info = コンパイル済みの関数の情報が入っている
/// - module = 新しく追加する
pub fn add_module(
        native_info: &mut Vec<abi::NativeFuncData>,
        module: &abi::NativeFuncData
) -> bool {
    // 既にあるか検索
    if native_info.iter().find(|f| f.module_name == module.module_name).is_some() {
        println!("already have this module.");
        return false;
    }

    // なければ追加
    native_info.push(module.clone());
    true
}
