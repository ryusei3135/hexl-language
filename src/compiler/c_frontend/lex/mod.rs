mod builtin_headers;
pub(in crate::compiler::c_frontend) mod lexer;
mod preproc;

pub(in crate::compiler::c_frontend) use lexer::*;
pub(in crate::compiler::c_frontend) use preproc::{PpError, Preprocessor};
