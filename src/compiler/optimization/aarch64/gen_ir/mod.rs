//! IRレベルの最適化(定数畳み込み・契約付きメモリ長の追跡)は、
//! ターゲットのCPUに依存しないので、x64のものをそのまま再公開する。
pub use super::super::x64::gen_ir::{constant_fold, tracking_contract_len};
