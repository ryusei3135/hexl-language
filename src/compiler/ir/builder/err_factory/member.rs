use crate::compiler::err::CompileErrKind;
use crate::compiler::err::compile::CompileErr;
use crate::compiler::err::{self, undef::UndefKind::*};

use std::rc::Rc;

/// ===================
/// member_is_arr_ref
/// ====================

/// 配列のしていされたものが数字じゃなかった
pub(in crate::compiler::ir::builder) fn arr_index_is_not_num<R>(
    val: &str,
    span: &Rc<err::Span>
) -> Result<R, err::ErrKind> {
    crate::GenCompileErr2!(CompileErrKind::InvalidIndexType {
        val: val.to_owned()
    }, Rc::clone(&span))
}

/// 構造体が存在しなかった
pub(in crate::compiler::ir::builder) fn this_struct_is_undefined(
    found: &str,
) -> Result<(), err::ErrKind> {
    Err(crate::GenUndefErrResult!(
        UndefStruct,
        found.to_owned(),
        None
    ))
}

/// 構造体のメンバーが存在しなかった
pub(in crate::compiler::ir::builder) fn this_member_is_not_found_struct(
    found: &str,
) -> Result<(), err::ErrKind> {
    Err(crate::GenUndefErrResult!(
        UndefMemberInVar,
        found.to_owned(),
        None
    ))
}
