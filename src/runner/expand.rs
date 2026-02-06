use super::*;


///  渡されたノードを配列のVarValueに変換
///  ノードは分岐する構造のため、この関数を再起することでリストを作成する
pub fn expand_array_node(
        node: node::CalculNode
) -> Vec<Box<type_info::VarValue>> {
    let mut array_value = Vec::<Box::<type_info::VarValue>>::new();
    if node.node_type == node::NodeKind::NodeArray {
        // right_nodeに次のリストの値が入っている
        array_value = expand_array_node(*node.right_node.clone().unwrap());
        array_value.push(Box::new(eval::node_run(*node.left_node.clone().unwrap())));
    }

    array_value
}
