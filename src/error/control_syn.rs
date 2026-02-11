
#[derive(PartialEq, Clone, Debug)]
pub enum ControlSynErr {
    /// エラー
    DataIsNotFound,
    InvalidIterCond,
    ValueIsOfInvalidType,
    /// システムエラー
    MissingCondInForStatement,
    /// ログ
    EndLoop,
    SETTING,
}
