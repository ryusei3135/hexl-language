mod builder;
pub mod inst;

use builder::Size;
use crate::{
    err,
    node,
};

pub use builder::{
    IR,
    VarTree,
    FuncTree,
    FuncDefInfo,
};
