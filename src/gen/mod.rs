//! このモジュールは、IRで生成したものを
//! 渡されたフォーマットのアセンブリ言語に変換するApiを提供する

mod asm_emitter;
mod inline_asm;
mod call_func;
mod mng_fmt;

use crate::ir::{
    FuncTree,
    inst,
    Size,
};
use crate::{
    *,
};
use std::mem;
use std::collections::HashMap;

pub use asm_emitter::AsmEmitter;
