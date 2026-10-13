//! 符号なし整数(`UDB`/`UDW`/`UDD`/`UDQ`)の判定と、
//! 同じ幅の符号付き整数への変換
//!
//! レジスタの選択・ニーモニックのサイズ接尾辞は、符号の有無に関係なく
//! 「幅」だけで決まる。符号の有無で出力が変わるのは次の命令だけ。
//!
//! | 処理 | 符号付き | 符号なし |
//! |------|----------|----------|
//! | `<` `>` | `jl` / `jg` | `jb` / `ja` |
//! | `/` `%` | `cltd`+`idiv` | `xor %edx, %edx`+`div` |
//! | 添字の64bit化 | `movs?q` | `movz?q`(32bitは`movl`だけで上位が0になる) |
//!
//! `+` `-` `*`(2オペランドの`imul`)は下位ビットの結果が同じなので共通。

use crate::compiler::ir::types::Size;

pub(in crate::compiler::asm_gen) trait SizeSign {
    /// 符号なし整数(配列なら要素の型)かどうか
    fn is_unsigned(&self) -> bool;
    /// 同じ幅の符号付き整数にする(`UDD` -> `DD`)。
    /// 符号付きの型・ポインタなどはそのまま返す
    fn to_signed(&self) -> Size;
}

impl SizeSign for Size {
    fn is_unsigned(&self) -> bool {
        match self {
            Size::UDB | Size::UDW | Size::UDD | Size::UDQ => true,
            Size::Array { size, .. } => size.is_unsigned(),
            _ => false,
        }
    }

    fn to_signed(&self) -> Size {
        match self {
            Size::UDB => Size::DB,
            Size::UDW => Size::DW,
            Size::UDD => Size::DD,
            Size::UDQ => Size::DQ,
            other => other.clone(),
        }
    }
}
