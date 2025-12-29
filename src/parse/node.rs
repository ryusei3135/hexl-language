

#[derive(Clone, Debug, PartialEq)]
pub enum NodeKind {
    NodeNum,
    NodeStr,
    //  演算子
    NodeAdd,
    NodeSub,
    NodeMul,
    NodeDiv,

    NodeNull,
    //  変数
    NodeCallVar,
    NodeAssignVar,
    NodeVarName,
    NodeDefVar,
    //  型
    NodeType,
}

#[derive(Clone, Debug)]
pub struct CalculNode {
    pub value: String,
    pub node_type: NodeKind,
    pub left_node: Option<Box<CalculNode>>,
    pub right_node: Option<Box<CalculNode>>,
}

#[derive(Clone)]
pub struct FuncNode {
    //  関数の名前
    pub name: String,
    //  関数の処理
    pub nodes: Vec<CalculNode>,
}
