use super::*;


//  渡された引数の情報で変数を作成
pub fn make_args_var(
        args_node: func::FuncArgsNode,
        call_node: node::CalculNode
) {
    if args_node.name == "[*null*]".to_string() {
        return;
    }
    global_state::var_manager().add_var(
        args_node.name.clone(),
        run::node_run(*call_node.left_node.clone().unwrap()),
        VarRegion::Stack,
    );
    if let Some(arg) = args_node.next.clone() {
        if let Some(value) = call_node.right_node {
            make_args_var(*arg.clone(), *value.clone());
        }
    }
}
