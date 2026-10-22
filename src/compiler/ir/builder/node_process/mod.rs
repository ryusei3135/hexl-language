//! ノードからIRを作る処理のうち、`IR`のメソッドを役割ごとに分けたもの。
//!
//! | ファイル | 内容 |
//! |---|---|
//! | `data_node.rs` | データを扱う: 変数定義・配列の初期化と添字代入・構造体/バリアント型の初期化・列挙型のメンバー |
//! | `expr_stmt.rs` | 式を扱う: 型の推論・二項演算のオペランド型・単項演算の展開・キャスト・`++`/`--`・代入 |

mod data_node;
mod expr_stmt;

use std::rc::Rc;

use crate::compiler::hexl_frontend::parse::{self, VarMutAttr};

use super::*;
use crate::compiler::ir::result_mod::*;
use err_factory::expr_node;
