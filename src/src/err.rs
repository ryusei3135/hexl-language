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
pub enum LexErr {
    ThisNumIsInvalid
}

impl LexErr {
    pub fn fmt(self, line: &usize, pos: &usize) -> ErrKind {
        ErrKind::LexErrs {
            kind: self,
            line: line.clone(),
            pos: pos.clone(),
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
    MissingTknAfter(Option<lex::Tkn>),
    LexErrs {
        kind: LexErr,
        line: usize,
        pos: usize,
    } 
}

impl ErrKind {
    pub fn lex_err(&self) {
        if let Self::LexErrs { kind, line, pos } = self {
            match &kind {
                LexErr::ThisNumIsInvalid => {
                    println!("this num is invalid");
                    println!("line `{}`, pos `{}`", line, pos);
                }
            }
        }
    }
}
