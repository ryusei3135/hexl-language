use std::sync::{OnceLock, Mutex, MutexGuard};
use libloading::Symbol;
use crate::package::state::*;
use crate::package::load::state;
use std::collections::HashMap;


static C_FUNCS_TABLE: OnceLock<Mutex<HashMap<String, VmFn>>> = OnceLock::new();


//  関数のデータにアクセス
fn func_manager() -> MutexGuard<'static, HashMap<String, VmFn>> {
    C_FUNCS_TABLE
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap()
}

fn make_vm_value_args(args: &Vec<String>) -> Vec<VmValue> {
    let mut vm_args = Vec::<VmValue>::new();

    for arg in args {
        match arg.as_str() {
            "str" => vm_args.push(VmValue::Str("null".to_string())),
            "int" => vm_args.push(VmValue::Int(1)),
            "bool" => vm_args.push(VmValue::Bool(true)),
            "void" => vm_args.push(VmValue::Void),
            _ => panic!("c func type err"),
        }
    }

    vm_args
}

fn make_bridge(func: CFunc, def: state::FuncData) -> VmFn {
    let vm_args = make_vm_value_args(&def.args);

    Box::new(move |args: &[VmValue]| {
        // 引数個数チェック
        if args.len() != vm_args.len() {
            panic!("argument count mismatch");
        }
        VmValue::Args(vm_args.clone())
    })
}

pub fn add_c_func(name: String, c_func: Symbol<CFunc>, func_data: state::FuncData) {
    func_manager().insert(name, make_bridge(*c_func, func_data));
}
