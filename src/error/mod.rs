//! このモジュールはエラーログや処理中のログに対応
//! する列挙型などを書く
//! # 概要
//! - エラーログなどの列挙型を定義

/// 制御構文関連のログ
pub mod control_syn;
/// 変数や関数などの定義に関するログ
pub mod define_msg;
pub mod err_handling;
