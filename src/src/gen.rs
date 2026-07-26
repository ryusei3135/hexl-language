//! このモジュールは、IRで生成したものを
//! 渡されたフォーマットのアセンブリ言語に変換するApiを提供する


/// self.format_lineで構造体のポインタを渡すところがある
mod asm_emitter;
mod inline_asm;
mod call_func;
mod mng_fmt;

use crate::ir::{
    def_tree,
    inst,
    builder,
    types::Size,
};
use std::mem;
use std::collections::HashMap;

pub use asm_emitter::AsmEmitter;
