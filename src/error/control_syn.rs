
#[derive(PartialEq, Clone)]
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
