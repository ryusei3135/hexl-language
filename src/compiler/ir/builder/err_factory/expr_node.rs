use crate::compiler::err::{self, PreprocErrs::NotFoundAsmName, undef::UndefKind::*};

use crate::compiler::ir::inst;
use super::*;

/// 構造体が存在しなかった
pub(in crate::compiler::ir::builder) 
fn this_struct_is_undefined(
    found: &str,
) -> Result<inst::Inst, err::ErrKind> {
    Err(crate::GenUndefErrResult!(
        UndefStruct,
        found.to_owned(),
        None
    ))
}

/// バリアント型が存在しなかった
pub(in crate::compiler::ir::builder) 
fn this_variant_is_undefined(
    found: &str,
) -> Result<inst::Inst, err::ErrKind> {
    Err(crate::GenUndefErrResult!(
        UndefVariant,
        found.to_owned(),
        None
    ))
}

/// バリアント型に、そのメンバーが存在しなかった
pub(in crate::compiler::ir::builder) 
fn this_variant_field_is_undefined(
    found: &str,
) -> Result<inst::Inst, err::ErrKind> {
    Err(crate::GenUndefErrResult!(
        UndefVariantField,
        found.to_owned(),
        None
    ))
}
