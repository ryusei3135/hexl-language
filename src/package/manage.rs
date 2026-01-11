use std::ffi::{CString, c_char};
use std::sync::{OnceLock, Mutex, MutexGuard};
use crate::package::abi;
use crate::parse::node;
use crate::parse::resp::handler;
use crate::manager::global_state;
use crate::manager::variable;
use crate::lib;


static NATIVE_FUNC_DATA: OnceLock<Mutex<Vec<abi::NativeFuncData>>> = OnceLock::new();


//  関数のデータにアクセス
fn native_func_manager() -> MutexGuard<'static, Vec<abi::NativeFuncData>> {
    NATIVE_FUNC_DATA
        .get_or_init(|| Mutex::new(Vec::<abi::NativeFuncData>::new()))
        .lock()
        .unwrap()
}

fn make_receiver(module: &abi::NativeFuncData) {
    global_state::var_manager().add_var(
        module.module_name.clone(),
        handler::convert_value_to_node(
            module.module_name.clone(),
            node::NodeKind::NodeReceiver,
        ),
        "[*null*]".to_string(),
    );

    for (key, func) in &module.func {
        let method = handler::make_method_node(key.clone(), node::NodeKind::NodeNativeFunc);
        global_state::var_manager().add_method(method.clone(), key.to_string());
    }
}

///  ネイティブ関数に渡す引数を作成
fn make_vm_args(args: node::CalculNode) -> Vec<lib::VmArgsValue> {
    let mut vm_args = Vec::<lib::VmArgsValue>::new();
    if args.node_type == node::NodeKind::NodeArgsValue {
        vm_args.push(
            lib::VmArgsValue {
                arg_type: lib::ArgType::Str,
                value: unsafe {
                    lib::CValue {
                        str_value: CString::new((*args.left_node.clone().unwrap()).value.clone()).unwrap().into_raw(),
                    }
                }
            }
        )
    }
    return vm_args;
}

pub fn run_native_func(receiver_name: String, func_name: String, args: node::CalculNode) -> String {
    let func = native_func_manager()
        .iter()
        .find(|n| n.module_name == receiver_name)
        .map(|n| n.clone())
        .unwrap_or_else(|| {
            panic!("this func is not found(lib)");
        });
    let vm_func = func.func
        .get(&func_name)
        .expect("this netive func is false");
    unsafe {
        let vm_args = make_vm_args(args.clone());
        vm_func(vm_args.as_ptr() as *mut lib::VmArgsValue, vm_args.len());
    }
    "end".to_string()
}

pub fn add_module(module: abi::NativeFuncData) -> bool {
    let mut manager = native_func_manager(); // &mut Vec<NativeFuncData> など

    // 既にあるか検索
    if manager.iter().any(|f| f.module_name == module.module_name) {
        println!("already have this module.");
        return false;
    }

    make_receiver(&module);

    // なければ追加
    manager.push(module);
    true
}
