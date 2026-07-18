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

/// プリプロセッサ関係のエラーコード
#[derive(Clone, Debug, PartialEq)]
pub enum PreprocErrs {
    // #asm
    ExpectedLParenAfterAsm,
    ExpectedRParenAfterAsm,
    NotFoundAsmName,

    // #include
    ExpectedPathSegment,
}

impl PreprocErrs {
    pub fn build(self, line: &usize) -> ErrKind {
        ErrKind::PreprocErr {
            kind: self,
            line: line.clone()
        } 
    }
}


#[derive(Clone, Debug, PartialEq)]
pub enum ErrKind {
    EndTkn,
    OptErr,
    SystemErr(SystemErr),
    UnexpectedToken,
    NotFoundTkn(lex::Tkn),
    SyntaxErr(SyntaxErr),
    PreprocErr {
        kind: PreprocErrs,
        line: usize,
    },
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
