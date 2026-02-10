use crate::manager::*;


#[derive(Clone, Debug, PartialEq)]
pub enum NodeKind {
    NodeNum,
    NodeFloat,
    NodeBoolTrue,
    NodeBoolFalse,
    NodeStr,
    //  演算子
    NodeAdd,
    NodeSub,
    NodeMul,
    NodeDiv,
    NodeModulo,
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
    NodeArray,
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
    NodeFor,
    NodeRet,
    NodeIn,
    NodeRangeOp,
    //  参照
    NodeStd,
    NodeSrc,
    NodeCallModule,
    NodeNativeFunc,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CalculNode {
    pub value: String,
    pub node_type: NodeKind,
    pub left_node: Option<Box<CalculNode>>,
    pub right_node: Option<Box<CalculNode>>,
    //  どこのブロックの中か
    pub block: Option<usize>
}


#[derive(Clone)]
pub struct AllInfo {
    pub var_info: variable::VariableManager,
    pub func_info: func::FuncManager,
}

impl AllInfo {
    pub fn new() -> Self {
        Self {
            var_info: variable::VariableManager::new(),
            func_info: func::FuncManager::new(),
        }
    }
}
