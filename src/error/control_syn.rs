
#[derive(PartialEq)]
pub enum ControlSynErr {
    /// エラー
    InvalidIterCond,
    DataIsNotFound,
    ValueIsOfInvalidType,
    /// ログ
    EndLoop,
    SETTING,
}
