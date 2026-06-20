//! このモジュールは、IRで生成したものを
//! 渡されたフォーマットのアセンブリ言語に変換するApiを提供する

mod asm_emitter;
mod inline_asm;

use crate::ir::{
    FuncTree,
};
use std::collections::HashMap;

pub use asm_emitter::AsmEmitter;
