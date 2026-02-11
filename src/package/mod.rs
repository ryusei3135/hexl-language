pub mod load;
pub mod abi;
pub mod manage;
pub mod yaml;
pub mod native;


use std::ffi::{CString, CStr};
use crate::manager::type_info;
use crate::parse::node;
use crate::lib;
use crate::runner::eval;
use crate::runner::run;
use std::path::PathBuf;
