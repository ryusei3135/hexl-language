use super::*;


/// ノードから、引数の値をスタック領域に生成
/// #1~2
/// この順番を変えてはいけない
/// 引数に変数を使った場合、#1で、引数に使う変数のスコープがなくなる
pub fn make_args_var(
        runtime: &mut run::Runtime,
        args_node: &func::FuncArgsNode,
        call_node: &node::CalculNode
) {
    // #1
    let arg_value = eval::node_run(runtime, &args_node.type_name, *call_node.left_node.clone().unwrap());
    // #2
    if args_node.next == None {
        runtime.all_info.var_info.make_scope();
        runtime.all_info.var_info.make_new_stack();
    }

    if let Some(arg) = args_node.next.clone() {
        if let Some(value) = &call_node.right_node {
            make_args_var(
                runtime,
                &arg,
                &value
            );
        }
    }
    // 変数を作成
    runtime.all_info.var_info.add_var(
        &args_node.name,
        arg_value,
        &args_node.type_name,
        VarRegion::Stack,
        args_node.multiple.clone(),
    );
}
