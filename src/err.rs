use crate::lex;


#[derive(Clone, Debug, PartialEq)]
pub enum SystemErr {
    FlagNotFound,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SyntaxErr {
    Expected(char),
    Unexpected(lex::Tkn),
    DoubleTokenErr(lex::Tkn),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ErrKind {
    EndTkn,
    SystemErr(SystemErr),
    UnexpectedToken,
    NotFoundTkn(lex::Tkn),
    SyntaxErr(SyntaxErr),
}


impl ErrKind {
    pub fn gen(self, line: &usize, pos: &usize) -> Errs {
        Errs {
            line: line.clone(),
            pos: pos.clone(),
            kind: self,
        }
    }
}


#[derive(Clone, Debug, PartialEq)]
pub struct Errs {
    line: usize,
    pos: usize,
    kind: ErrKind,
}

impl Errs {
    pub fn print_log(&self, contents: &String) {
        let line = contents.lines().nth(self.line - 1).unwrap();

        println!("[err] line {}, pos: {}", self.line, self.pos);
        println!("{:?}: {:?}", self.kind, line);
    }
}
