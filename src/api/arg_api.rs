use super::*;


/// ノードから、引数の値をスタック領域に生成
pub fn make_args_var(
        runtime: &mut run::Runtime,
        args_node: &func::FuncArgsNode,
        call_node: &node::CalculNode
) {
    if *args_node.name == "[*null*]".to_string() {
        return;
    }
    let arg_value = eval::node_run(runtime, &args_node.type_name, *call_node.left_node.clone().unwrap());
    runtime.all_info.var_info.add_var(
        &args_node.name,
        arg_value,
        &args_node.type_name,
        VarRegion::Stack,
    );
    if let Some(arg) = args_node.next.clone() {
        if let Some(value) = &call_node.right_node {
            make_args_var(runtime, &arg, &value);
        }
    }
}
