use crate::lex;


#[derive(Clone, Debug, PartialEq)]
pub enum SystemErr {
    FlagNotFound,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SyntaxErr<'a> {
    UnmatchNumberSize {
        expect: &'a str,
        found: &'a str,
        msg: Option<&'a str>,
    },
    NotFoundNode(&'a str),
    UndefineMnemonic(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Err<'a> {
    SystemErr(SystemErr),
    UnexpectedToken(String),
    MissingToken(lex::Tkn),
    SyntaxErr(SyntaxErr<'a>),
    SyntaxErrTyNotMatch,
    SyntaxErrNotFoundTkn,
}
