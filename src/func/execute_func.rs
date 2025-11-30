
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

//  制御構文が実行可能か調べる
//  -2を返すと、その中の処理をすべてスキップする
unsafe fn match_control_expr(data: *mut func::FuncBlock, run_number: i32) -> i32 {
    let result: i32 = match &(*(*(*data).process.add(run_number as usize)).process_ptr).op_type {
        func::OpType::OpIf | func::OpType::OpIfElse | func::OpType::OpElse => {
                func::check_cond_expr(data, run_number as c_int) as i32
            }
        //  ここまで、条件分岐
        func::OpType::OpLoop => {
                if func::calcul_eval(
                    (*(*data).process
                        .add(run_number as usize))
                        .process_ptr) != 0 {
                    return 4;
                } else {
                    return -2;
                }
            }
        _ => {
            func::execute_one_line((*data).process, run_number as c_int);
            -1
        }
    };
    return result;
}

//  反復処理を実行
unsafe fn iterate_process(data: *mut func::FuncBlock, start: i32, end_indent: i32) -> i32 {
    let mut result: i32;
    //  反復処理の条件を除くため、1を足す
    let mut count: i32 = start + 1;

    while (*(*(*data).process.add(count as usize)).process_ptr).indent_len as i32 != end_indent {
        result = match_control_expr(data, count);
        //  制御構文の条件が,falseのため、中の処理をすべてスキップ
        if result == -2 {
            //  -2なので、インデントの中のすべての処理をスキップ
            count = func::skip_next_len_indent(data, count as c_int) as i32;
            continue;
        } else if result == 4 {
            count = iterate_process(
                    data,
                    count,
                    (*(*(*data).process.add(count as usize)).process_ptr).indent_len as i32
                ) as i32;
        }
        count += 1;
        //  反復処理の条件に戻る
        if (*(*(*data).process.add(count as usize)).process_ptr).indent_len as i32 == end_indent {
            if func::calcul_eval((*(*data).process.add(start as usize)).process_ptr) != 0 {
                count = start + 1;
            } else {
                break;
            }
        }
    }

    return count;
}

//  関数を実行
unsafe fn execute_func_process(data: *mut func::FuncBlock) {
    //  処理が制御構文の場合、結果を代入する変数
    let mut result: i32;
    let mut run_number: i32 = 0;

    while run_number < (*data).process_length as i32 {
        result = match_control_expr(data, run_number);
        //  制御構文の条件が,falseのため、中の処理をすべてスキップ
        if result == -2 {
            //  -2なので、インデントの中のすべての処理をスキップ
            run_number = func::skip_next_len_indent(data, run_number as c_int) as i32;
            continue;
        } else if result == 4 {
            run_number = iterate_process(
                data,
                run_number,
                (*(*(*data).process.add(run_number as usize)).process_ptr).indent_len as i32
            ) as i32;
        }
        run_number += 1;
    }
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
        //  ======================
        //  関数の処理データを実行
        //  ======================
        execute_func_process(data);
    } else {
        //  外部の関数を呼び出す
        func::current_func_name(caller_func);
        return func::eval_lib_func((*call_data).func_name, (*call_data).lib_header, args);
    }
    //  変数のアクセス特権を戻す
    func::current_func_name(caller_func);
    return 1;
}
