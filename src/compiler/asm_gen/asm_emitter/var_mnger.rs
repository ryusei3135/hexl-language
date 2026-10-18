use super::*;


#[derive(Debug, Clone)]
pub struct VarIndexInfo {
    /// レジスタ番号。`is_stack`が`true`の変数(ポインタでない構造体の
    /// 変数)はレジスタを持たないので[`VarIndexInfo::NO_REG`]が入る
    /// (`%rbp`からの位置は`offset`を使う)
    pub reg: RegisterMode,
    /// `is_stack`が`true`の場合の、`%rbp`からのバイトオフセット
    /// (構造体先頭のメンバーの位置)。`is_stack`が`false`の場合は`0`で意味を持たない
    pub offset: usize,
    pub size: types::Size,
    pub index: usize,
    pub is_stack: bool,
    /// 配列変数の場合の、先頭要素を置く直前の`stk_use_counter`
    /// (`%rbp`からのバイトオフセット)。要素`i`は
    /// `%rbp - (arr_base + (i + 1) * 要素サイズ)`に置かれる
    /// (要素0が最も`%rbp`に近い)。配列以外では`0`
    pub arr_base: usize,
    /// スタック上のメモリ(配列など、`mem_val_ir`で確保した領域)に
    /// 実体がある変数かどうか。`true`の場合、`reg`はレジスタ番号として
    /// 使われていないので、レジスタの使用中の管理や、インラインアセンブラの
    /// レジスタ退避の対象にしてはいけない
    pub in_mem: bool,
}

impl VarIndexInfo {
    /// レジスタを持たない変数(`is_stack`)の`reg`に入れる値。
    /// レジスタ番号として使うと範囲外になって即座にpanicするので、
    /// オフセットをレジスタ番号と取り違えるバグを見逃さない
    pub const NO_REG: usize = usize::MAX;

    pub fn new(reg: RegisterMode, size: &types::Size, index: usize) -> Self {
        Self {
            reg: reg,
            offset: 0,
            size: size.clone(),
            index: index,
            is_stack: false,
            arr_base: 0,
            in_mem: false,
        }
    }

    /// ポインタでない構造体の変数を、`%rbp`からのオフセット(`offset`)に
    /// 直接置かれている変数として登録する。レジスタは使わない
    pub fn new_stack(offset: usize, size: &types::Size, index: usize) -> Self {
        Self {
            reg: RegisterMode::Normal(Self::NO_REG),
            offset,
            size: size.clone(),
            index: index,
            is_stack: true,
            arr_base: 0,
            in_mem: false,
        }
    }

    /// 配列の先頭の位置(`%rbp`からのオフセット)を記録する
    pub fn with_arr_base(mut self, arr_base: usize) -> Self {
        self.arr_base = arr_base;
        self.in_mem = true;
        self
    }
}

impl AsmEmitter {
    /// 指定された名前の変数の型を返す
    pub(in crate::compiler::asm_gen) fn get_var_ty(&self, var_name: &str) -> Size {
        self.var_info(var_name).size.clone()
    }

    #[inline(always)]
    pub(in crate::compiler::asm_gen) fn update_value_info(&mut self, name: &str, index: usize) {
        self.var_info_mut(name).index = index;
    }

    #[inline(always)]
    pub(in crate::compiler::asm_gen) fn update_value_reg(
        &mut self,
        name: &str,
        reg: &RegisterMode,
    ) {
        self.used_reg.mark_used(reg);
        self.var_info_mut(name).reg = reg.clone();
    }

    /// レジスタに載せる値の型`ty`から、実際に使うレジスタの
    /// サイズを決める。
    ///
    /// - `DB`/`DW`/`DD`/`DQ`: そのままのサイズ
    /// - 配列: 要素のサイズ
    /// - ポインタ/構造体など: アドレスを持つので64bit(`DQ`)
    pub(in crate::compiler::asm_gen) fn value_reg_size(ty: &Size) -> Size {
        match ty {
            Size::DB | Size::DW | Size::DD | Size::DQ | Size::F32 | Size::F64 => ty.clone(),
            // 符号なしは、同じ幅の符号付きと同じレジスタを使う
            Size::UDB | Size::UDW | Size::UDD | Size::UDQ => ty.to_signed(),
            Size::Array { size, .. } => Self::value_reg_size(size),
            _ => Size::DQ,
        }
    }
}