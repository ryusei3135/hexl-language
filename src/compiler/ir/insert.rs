//! `checker`が見つけた「実行時に必要な検査」を、IRに条件式のノードとして
//! 差し込む(insertする)モジュール
//!
//! - `checker`: 静的に分かることは検査し、実行時に判定が必要なものは
//!   「何を差し込むべきか」を表す値(例: `checker::ArrIdxGuard`)として返す
//! - `insert`: その値を受け取り、条件分岐のIR(`ExpectJmp`/`Jmp`/`Block`と
//!   比較式)を`ir_tree`へ積む
//!
//! 検査(checker)とIRの生成(insert)を分けることで、`builder`は
//! 「ガードが返ってきたら`insert`を呼ぶ」だけで済む。
//!
//! ## ファイル構成
//! | ファイル | 役割 |
//! | :--- | :--- |
//! | `emit.rs` | 命令・比較式を`ir_tree`へ積む小さなAPI(`push_inst`など) |
//! | `array_guard.rs` | `[arr idx else 0]`の範囲外ガード |
//!
//! このモジュールは`ir.rs`で`mod insert;`として宣言する。

use super::*;

mod array_guard;
mod emit;
