use crate::compiler::err;

use super::*;

#[derive(Debug, PartialEq)]
pub struct ErrPool {
    err_list: Vec<ErrKind>,
}

impl ErrPool {
    pub fn new() -> Self {
        Self {
            err_list: Vec::new(),
        }
    }

    pub fn push_err_node(&mut self, err_node: ErrKind) {
        self.err_list.push(err_node);
    }

    pub fn view(&self) {
        for e in self.err_list.iter() {
            match e {
                ErrKind::Syntax(n) => {
                    Self::view_syntax_err(n);
                }
                _ => {}
            }
        }
    }

    fn view_syntax_err(n: &err::SyntaxErr) {
        match &n.kind {
            err::SyntaxErrKind::TknIsEof { expected } => {
                println!("[ERR]>>{}:{}:{}", n.file_name, n.loc.span.line, n.loc.span.pos);
            }
            err::SyntaxErrKind::UnenclosedScope { target } => {
                //
            }
            err::SyntaxErrKind::UnexpectTknInExpr { found } => {
                //
            }
            _ => {}
        }
    }
}