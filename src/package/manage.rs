use super::*;


static NATIVE_FUNC_DATA: OnceLock<Mutex<Vec<abi::NativeFuncData>>> = OnceLock::new();


//  関数のデータにアクセス
fn native_func_manager() -> MutexGuard<'static, Vec<abi::NativeFuncData>> {
    NATIVE_FUNC_DATA
        .get_or_init(|| Mutex::new(Vec::<abi::NativeFuncData>::new()))
        .lock()
        .unwrap()
}

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
                "void" => return vm_args,
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
            lib::ArgType::Bool => Some(type_info::VarValue::Bool(ret_value.value.bool_value)),
            lib::ArgType::Void => None,
        };
    }
}

pub fn run_native_func(
        runtime: &mut run::Runtime,
        receiver_name: String,
        func_name: String,
        args: node::CalculNode
) -> Option<type_info::VarValue> {
    let func = native_func_manager()
        .iter()
        .find(|n| n.module_name == receiver_name)
        .map(|n| n.clone())
        .unwrap_or_else(|| {
            panic!("this func is not found(lib)");
        });
    let func_call_data = func.func
        .get(&func_name)
        .expect("this netive func is false");
    let vm_func = func_call_data.func_ptr;
    let ret_value = unsafe {
        let vm_args = make_vm_args(runtime, args.clone(), func_call_data.args_type.clone());
        let ret = vm_func(vm_args.as_ptr() as *mut lib::VmArgsValue, vm_args.len());
        get_vm_ret_value(ret)
    };
    ret_value
}

pub fn add_module(module: abi::NativeFuncData) -> bool {
    let mut manager = native_func_manager(); // &mut Vec<NativeFuncData> など

    // 既にあるか検索
    if manager.iter().any(|f| f.module_name == module.module_name) {
        println!("already have this module.");
        return false;
    }

    // なければ追加
    manager.push(module);
    true
}
