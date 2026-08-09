use std::collections::HashMap;
use std::{
    mem,
};
use crate::node;

pub mod inst;
pub mod builder;
/// 変数や関数、構造体などの、定義を一時的に
/// 保存する構造体を提供するモジュール
pub mod def_tree;

pub mod types;

pub use builder::{
    IR,
};
use crate::{
    err,
};
