pub mod func;
pub mod package_node;
pub mod cond_branch;
pub mod ret;
pub mod for_node;

use crate::parse::expr;
use crate::parse::resp;
use crate::parse::node;

use crate::token::token;
use crate::api::type_api;
use crate::manager;
use crate::parse::expr::parse_type;
