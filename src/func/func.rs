use std::os::raw::c_char;
use std::ffi::c_int;


#[repr(C)]
#[derive(PartialEq)]
pub enum  OpType {
    Add,
    Sub,
    Mul,
    Div,

    Num,
    String,
    CallFunc,
    AssignVar,
    CallVar,
    //  ===  関係演算子 ===
    TypeOpEpual,
    TypeOpBigger,
    TypeOpSmallerThen,
    TypeOpHigher,
    TypeOpBelow,
    TypeOpIsNot,
    //  === 論理演算子 ===
    TypeOpAnd,
    TypeOpOr,
    // === 制御構文 ===
    OpIf,
    OpIfElse,
    OpElse,
    OpLoop,
    OpRet,
}

//  この構造体のポインタの最初は、必ずargs.lengthが来る
#[repr(C)]
pub union ArgsNode {
    pub length: c_int,
    pub name: *mut c_char,
    pub value: *mut CalculNode,
}

//  関数を呼ぶ際に、関数の名前や、ライブラリの場合
//  どのライブラリなのかを格納
#[repr(C)]
pub struct CallFuncNode {
    pub func_name: *mut c_char,
    pub lib_header: *mut c_char,
}

//  CalculNode
#[repr(C)]
pub union CalculNodeValue {
    pub value: *mut c_char,
    pub call_data: *mut CallFuncNode,
}

#[repr(C)]
pub union CalculNodeLeft {
    pub args: *mut ArgsNode,
    pub left: *mut CalculNode,
}

#[repr(C)]
pub struct CalculNode {
    pub indent_len: c_int,
    pub value: CalculNodeValue,
    pub r#op_type: OpType,
    pub left: CalculNodeLeft,
    pub right: *mut CalculNode,
}

#[repr(C)]
pub struct ProcessList {
    pub process_ptr: *mut CalculNode,
}

#[repr(C)]
pub struct FuncBlock {
    pub name: *mut c_char,
    pub process: *mut ProcessList,
    pub process_length: c_int,
    pub args: *mut ArgsNode,
}


//  src/variable/variable.hで定義
//  変数を追加 & 上書き
extern "C" {
    pub fn add_variable_value(
            name: *mut c_char,
            access_priv: *mut c_char,
            value: *mut CalculNode) -> c_int;
    //  src/extern_lib/load.hで定義
    pub fn eval_lib_func(
            name: *mut c_char,
            lib_header: *mut c_char,
            args: *mut ArgsNode) -> c_int;
    //  処理のデータが入っているノードを実行
    pub fn calcul_eval(n: *mut CalculNode) -> c_int;
    //  処理の一行を実行
    pub fn check_cond_expr(data: *mut FuncBlock, process_num: c_int) -> c_int;
    pub fn skip_next_len_indent(data: *mut FuncBlock, pos: c_int) -> c_int;
    pub fn execute_one_line(node: *mut ProcessList, pos: c_int);

    pub fn current_func_name(func_name: *mut c_char) -> *mut c_char;
    pub fn get_func_data(func_name: *mut c_char) -> *mut FuncBlock;
}
