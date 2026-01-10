use std::sync::{OnceLock, Mutex, MutexGuard};
use crate::package::abi;
use crate::parse::node;
use crate::parse::resp::handler;
use crate::manager::global_state;
use crate::manager::variable;


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
