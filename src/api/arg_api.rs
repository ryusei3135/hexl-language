use super::*;


//  渡された引数の情報で変数を作成
pub fn make_args_var(
        args_node: func::FuncArgsNode,
        call_node: node::CalculNode
) {
    global_state::var_manager().add_var(
        args_node.name.clone(),
        *call_node.left_node.clone().unwrap(),
        args_node.type_name.clone().unwrap().value
    );
    if let Some(arg) = args_node.next.clone() {
        if let Some(value) = call_node.right_node {
            make_args_var(*arg.clone(), *value.clone());
        }
    }
}
