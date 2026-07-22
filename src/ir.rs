use std::collections::HashMap;
use std::{
    mem,
};
use crate::node;

pub mod inst;
pub mod builder;

pub mod types;

pub use builder::{
    IR,
    FuncTree,
    FuncDefMetaData,
};
