

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
    NodeLessThan,
    NodeGreaterThan,
    NodeEqTo,
    NodeNotEqTo,
    NodeLessThanOrEqualTo,
    NodeGreaterThanOrEqualTo,

    NodeNot,

    NodeNull,
    //  変数
    NodeCallVar,
    NodeAssignVar,
    NodeVarName,
    NodeDefVar,
    NodeReceiver,
    //  型
    NodeType,
    //  関数
    NodeCallFunc,
    NodeArgsValue,
    //  処理
    NodeUsePackage,
    NodeIf,
    NodeIfElse,
    NodeElse,
    NodeRet,
    //  参照
    NodeStd,
    NodeSrc,
    NodeNativeFunc,
}

#[derive(Clone, Debug)]
pub struct CalculNode {
    pub value: String,
    pub node_type: NodeKind,
    pub left_node: Option<Box<CalculNode>>,
    pub right_node: Option<Box<CalculNode>>,
    //  どこのブロックの中か
    pub block: Option<i32>
}
