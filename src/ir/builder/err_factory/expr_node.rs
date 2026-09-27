use crate::err::{self, PreprocErrs::NotFoundAsmName, undef::UndefKind::*};

use crate::ir::inst;
use super::*;

pub(in crate::ir::builder)
fn this_enum_is_undefined(name: &str) -> Result<inst::Inst, err::ErrKind> {
    Err(crate::GenUndefErrResult!(
        UndefEnum,
        name.to_owned(),
        None
    ))
}

pub(in crate::ir::builder)
fn this_enum_member_is_undefined(name: &str) -> Result<inst::Inst, err::ErrKind> {
    Err(crate::GenUndefErrResult!(
        UndefEnumMember,
        name.to_owned(),
        None
    ))
}


/// 構造体が存在しなかった
pub(in crate::ir::builder) 
fn this_struct_is_undefined(
    found: &str,
) -> Result<inst::Inst, err::ErrKind> {
    Err(crate::GenUndefErrResult!(
        UndefStruct,
        found.to_owned(),
        None
    ))
}
