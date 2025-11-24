
mod func;

use std::ffi::{CStr};
use std::ffi::c_int;
use std::ffi::c_char;


//  c言語の文字列をrustで使える文字列に変換
fn convert_c_str<'a>(c_str: *const c_char) -> &'a str {
    unsafe {CStr::from_ptr(c_str)
        .to_str()
        .expect("Invalid UTF-8")}
}


//  関数の引数を展開
fn expand_args(def: *mut func::ArgsNode, value: *mut func::ArgsNode) {
    static S: &str = "[null]";
    let null_func_name: *const c_char = S.as_ptr() as *const c_char;
    //  引数を展開
    unsafe {
        for count in 0..(*def.wrapping_add(0)).length as usize {
            if count != 0 {
                func::add_variable_value(
                    (*def.add(count)).name,
                    func::current_func_name(null_func_name as *mut c_char),
                    (*value.add(count)).value
                );
            }
        }
    }
}


unsafe fn execute_process_judge(data: *mut func::FuncBlock, count: *mut i32) -> bool {
    if (*(*(*data).process.add(count as usize)).process_ptr).op_type == func::OpType::OpIf {
        println!("hello");
    }
    return true;
}

//  関数のデータをゲットし、実行
#[no_mangle]
pub unsafe extern "C" fn execute_func(
            call_data: *mut func::CallFuncNode,
            args: *mut func::ArgsNode,
            caller_func: *mut c_char) -> c_int {
    func::current_func_name((*call_data).func_name);

    if convert_c_str((*call_data).lib_header) == "[local]" {
    //  関数のデータを取得
        let data: *mut func::FuncBlock = func::get_func_data((*call_data).func_name);
        expand_args((*data).args, args);

        for count in 0..(*data).process_length as i32 {
            if execute_process_judge(data, count as *mut i32) {
                func::current_func_name(caller_func);

                func::execute_one_line((*data).process, count as c_int);
            }
        }
    } else {
        //  外部の関数を呼び出す
        func::current_func_name(caller_func);
        return func::eval_lib_func((*call_data).func_name, (*call_data).lib_header, args);
    }
    //  変数のアクセス特権を戻す
    func::current_func_name(caller_func);
    return 1;
}
