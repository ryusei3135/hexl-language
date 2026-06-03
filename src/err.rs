
#[derive(Clone, Debug, PartialEq)]
pub enum SystemErr {
    FlagNotFound,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Err {
    SystemErr(SystemErr),
    UnexpectedToken(String),
    SyntaxErr(String),
    SyntaxErrTyNotMatch,
    SyntaxErrNotFoundTkn,
}
