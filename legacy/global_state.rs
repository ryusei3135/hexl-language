// use super::*;


// static FUNC_STATE: OnceLock<Mutex<func::FuncManager>> = OnceLock::new();
// static VAR_STATE: OnceLock<Mutex<variable::VariableManager>> = OnceLock::new();


// //  関数のデータにアクセス
// pub fn func_manager() -> MutexGuard<'static, func::FuncManager> {
//     FUNC_STATE
//         .get_or_init(|| Mutex::new(func::FuncManager::new()))
//         .lock()
//         .unwrap()
// }

// //  変数のデータにアクセス
// pub fn var_manager() -> MutexGuard<'static, variable::VariableManager> {
//     VAR_STATE
//         .get_or_init(|| Mutex::new(variable::VariableManager::new()))
//         .lock()
//         .unwrap()
// }
