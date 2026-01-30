use super::*;

/// 反復処理(for文)の今の状態を返す戻り値の型
pub type IterStatus = Result<
    (
        bool,               // 反復処理が実行可能か
        type_info::VarValue,// 反復処理(for)のループ変数に代入する値
        ControlSemantics    //
    ),
    ControlSynErr
>;
