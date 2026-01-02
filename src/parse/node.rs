

#[derive(Clone, Debug, PartialEq)]
pub enum NodeKind {
    NodeNum,
    NodeStr,
    //  演算子
    NodeAdd,
    NodeSub,
    NodeMul,
    NodeDiv,
    //  比較演算子
    NodeEqTo,
    NodeNotEqTo,

    NodeNot,

    NodeNull,
    //  変数
    NodeCallVar,
    NodeAssignVar,
    NodeVarName,
    NodeDefVar,
    //  型
    NodeType,
    //  関数
    NodeCallFunc,
    NodeArgsValue,
    //  処理
    NodeUsePackage,
}

#[derive(Clone, Debug)]
pub struct CalculNode {
    pub value: String,
    pub node_type: NodeKind,
    pub left_node: Option<Box<CalculNode>>,
    pub right_node: Option<Box<CalculNode>>,
}

#[derive(Clone)]
pub struct FuncArgsNode {
    pub name: String,
    pub type_name: String,
}

#[derive(Clone)]
pub struct FuncNode {
    //  関数の名前
    pub name: String,
    pub args: Vec<FuncArgsNode>,
    pub ret_value_type: CalculNode,
    //  関数の処理
    pub nodes: Vec<CalculNode>,
}
