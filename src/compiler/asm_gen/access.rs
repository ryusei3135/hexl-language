//! `AsmEmitter`が持つ状態(変数の表・IR・静的領域のラベル)や、
//! アセンブリのフォーマッタ(`asm_fmt`)へのアクセスを、
//! 意味のある名前の関数にまとめたモジュール
//!
//! 以前は`self.var_hash_map.get(..).unwrap()`や
//! `self.asm_fmt.get_opcode_tmpl("mov").replace("{dst}", ..).replace("{src1}", ..)`
//! のような同じ書き方が各所に散らばっていた。ここに集めることで、
//! 呼び出し側は「何をしたいか」だけを書けばよくなる。
//!
//! ## 一覧
//! | 分類 | 関数 |
//! |---|---|
//! | IRのノード | [`inst_at`](AsmEmitter::inst_at) |
//! | 変数 | [`var_info`](AsmEmitter::var_info) / [`find_var_info`](AsmEmitter::find_var_info) / [`var_info_mut`](AsmEmitter::var_info_mut) / [`var_size`](AsmEmitter::var_size) / [`var_reg`](AsmEmitter::var_reg) / [`var_operand`](AsmEmitter::var_operand) / [`arr_elem_ty`](AsmEmitter::arr_elem_ty) / [`is_reg_held_by_var`](AsmEmitter::is_reg_held_by_var) |
//! | 構造体 | [`struct_member_offset`](AsmEmitter::struct_member_offset) / [`self_ptr_reg`](AsmEmitter::self_ptr_reg) |
//! | 静的領域 | [`static_label`](AsmEmitter::static_label) |
//! | メモリ参照 | [`rbp_ref`](AsmEmitter::rbp_ref) / [`ref_base_offset`](AsmEmitter::ref_base_offset) / [`reg64`](AsmEmitter::reg64) / [`ptr_deref_operand`](AsmEmitter::ptr_deref_operand) |
//! | 命令の1行 | [`fill_tmpl`](AsmEmitter::fill_tmpl) / [`tmpl_line`](AsmEmitter::tmpl_line) / [`mov_line`](AsmEmitter::mov_line) |

use super::*;

/// スタックフレームのベースレジスタ
pub(in crate::compiler::asm_gen) 
const FRAME_BASE_REG: &str = "%rbp";

// ===========================================================================
// IRのノード
// ===========================================================================
impl AsmEmitter {
    /// `idx`番目のIRのノードを複製して取得する
    ///
    /// `self.curr_inst[idx].clone()`と同じ。複製して返すので、
    /// 受け取った側が`&mut self`のメソッドを呼んでも借用が衝突しない。
    #[inline(always)]
    pub(in crate::compiler::asm_gen) 
    fn inst_at(&self, idx: usize) -> inst::Inst {
        self.curr_inst[idx].clone()
    }
}

// ===========================================================================
// 変数
// ===========================================================================
impl AsmEmitter {
    /// 変数の情報を取得する。登録されていなければpanicする
    pub(in crate::compiler::asm_gen) 
    fn var_info(&self, name: &str) -> &asm_emitter::VarIndexInfo {
        self.var_hash_map
            .get(name)
            .unwrap_or_else(|| panic!("this var is not found -> {}", name))
    }

    /// 変数の情報を取得する。登録されていなければ`None`
    pub(in crate::compiler::asm_gen) 
    fn find_var_info(
        &self,
        name: &str,
    ) -> Option<&asm_emitter::VarIndexInfo> {
        self.var_hash_map.get(name)
    }

    /// 変数の情報を書き換えるために取得する。登録されていなければpanicする
    pub(in crate::compiler::asm_gen) 
    fn var_info_mut(
        &mut self,
        name: &str,
    ) -> &mut asm_emitter::VarIndexInfo {
        self.var_hash_map
            .get_mut(name)
            .unwrap_or_else(|| panic!("this var is not found -> {}", name))
    }

    /// 変数の型。登録されていなければ`None`
    pub(in crate::compiler::asm_gen) 
    fn var_size(&self, name: &str) -> Option<Size> {
        self.find_var_info(name).map(|var| var.size.clone())
    }

    /// 変数がレジスタに置かれている場合のレジスタ番号
    /// (未登録、または`%rbp`相対のメモリに置かれている場合は`None`)
    pub(in crate::compiler::asm_gen) 
    fn var_reg(&self, name: &str) -> Option<usize> {
        self.find_var_info(name)
            .filter(|var| !var.is_stack)
            .map(|var| var.reg)
    }

    /// レジスタ`reg`を、いずれかの変数が保持しているか
    pub(in crate::compiler::asm_gen) 
    fn is_reg_held_by_var(&self, reg: usize) -> bool {
        self.var_hash_map
            .values()
            .any(|var| !var.is_stack && var.reg == reg)
    }

    /// 変数そのものを参照するオペランドを返す
    ///
    /// - `%rbp`相対のメモリに置かれた変数: `-8(%rbp)`のようなメモリ参照
    /// - レジスタに置かれた変数: そのレジスタ
    ///   (ポインタはアドレスを持つので常に64bit、それ以外は変数自身の型のサイズ)
    pub(in crate::compiler::asm_gen) 
    fn var_operand(&self, name: &str) -> String {
        let var = self.var_info(name);
        if var.is_stack {
            return self.rbp_ref(var.reg);
        }
        let size = if var.size.is_pointer().is_some() {
            Size::DQ
        } else {
            var.size.clone()
        };
        self.asm_fmt.get_fmt_reg(var.reg, &size)
    }

    /// 配列/ポインタの変数`name`が指す、要素1つの型
    /// (配列でもポインタでもなければ、変数自身の型)
    pub(in crate::compiler::asm_gen) 
    fn arr_elem_ty(&self, name: &str) -> Size {
        let var = self
            .find_var_info(name)
            .unwrap_or_else(|| panic!("array variable not found: {}", name));
        match var.size.clone() {
            Size::Array { size, .. } => *size,
            Size::Pointer { ty, .. } => *ty,
            ty => ty,
        }
    }
}

// ===========================================================================
// 構造体
// ===========================================================================
impl AsmEmitter {
    /// 構造体のメンバーの、構造体の先頭からのオフセット
    ///
    /// - `pos`: IRの、そのメンバーの終端までの累積サイズ
    /// - `member_size`: そのメンバー自身のサイズ
    ///
    /// 構造体のポインタは先頭のメンバーを指し、メンバーはそこから
    /// アドレスが小さくなる向きに並ぶので、`pos - member_size`になる。
    pub(in crate::compiler::asm_gen) 
    fn struct_member_offset(
        pos: usize,
        member_size: usize,
    ) -> usize {
        pos.saturating_sub(member_size)
    }

    /// 構造体のメソッドへ渡す、暗黙の`self`ポインタを持つレジスタ
    /// (第1引数のレジスタ。ポインタなので64bit)
    pub(in crate::compiler::asm_gen) 
    fn self_ptr_reg(&self) -> String {
        self.asm_fmt.get_fmt_param::<String>(0, &Size::DQ)
    }
}

// ===========================================================================
// 静的領域
// ===========================================================================
impl AsmEmitter {
    /// IRのノード`node_idx`の値を置いた、静的領域のラベル名
    pub(in crate::compiler::asm_gen) 
    fn static_label(&self, node_idx: usize) -> Option<String> {
        self.data_map
            .iter()
            .find(|(id, _)| *id == node_idx)
            .map(|(_, label)| label.clone())
    }
}

// ===========================================================================
// メモリ参照・レジスタ
// ===========================================================================
impl AsmEmitter {
    /// `%rbp`からのオフセットを参照するオペランド(`-8(%rbp)`など)
    pub(in crate::compiler::asm_gen) 
    fn rbp_ref(&self, offset: usize) -> String {
        self.asm_fmt.fmt_ref_operand(FRAME_BASE_REG, offset)
    }

    /// `base`レジスタからのオフセットを参照するオペランド
    /// (オフセットが0なら`(%rdi)`のようにオフセットを付けない)
    pub(in crate::compiler::asm_gen) 
    fn ref_base_offset(&self, base: &str, offset: usize) -> String {
        if offset == 0 {
            self.asm_fmt.fmt_ref_operand_no_offset(base)
        } else {
            self.asm_fmt.fmt_ref_operand(base, offset)
        }
    }

    /// レジスタ番号に対応する、64bitのレジスタ名
    pub(in crate::compiler::asm_gen) 
    fn reg64(&self, reg: usize) -> String {
        self.asm_fmt.get_fmt_reg(reg, &Size::DQ)
    }

    /// ポインタを持つレジスタ番号`reg`が指す先を、オフセット無しで参照する
    /// オペランド(`(%rdi)`など)
    pub(in crate::compiler::asm_gen) 
    fn ptr_deref_operand(&self, reg: usize) -> String {
        self.asm_fmt.fmt_ref_operand_no_offset(&self.reg64(reg))
    }
}

// ===========================================================================
// 命令の1行
// ===========================================================================
impl AsmEmitter {
    /// フォーマットファイルの`opcode`のテンプレートの`{dst}`と`{src1}`を
    /// 埋めた1行を返す(ニーモニックのサイズ接尾辞はまだ調整しない)
    pub(in crate::compiler::asm_gen) 
    fn fill_tmpl(
        &self,
        opcode: &str,
        dst: &str,
        src: &str,
    ) -> String {
        self.asm_fmt
            .get_opcode_tmpl(opcode)
            .replace("{dst}", dst)
            .replace("{src1}", src)
    }

    /// [`fill_tmpl`](Self::fill_tmpl)で埋めた1行の、ニーモニックのサイズ接尾辞を
    /// `size`に合わせて調整する
    ///
    /// `is_memory`が`true`(メモリを読み書きする命令)の場合は、
    /// `fmt.mnemonic_size`の設定に関係なく接尾辞を付ける。
    pub(in crate::compiler::asm_gen) 
    fn tmpl_line(
        &self,
        opcode: &str,
        dst: &str,
        src: &str,
        size: &Size,
        is_memory: bool,
    ) -> String {
        let line = self.fill_tmpl(opcode, dst, src);
        if is_memory {
            self.asm_fmt.fmt_memory_mnemonic_resize(opcode, &line, size)
        } else {
            self.asm_fmt.fmt_mnemonic_resize(opcode, &line, size)
        }
    }

    /// `mov`の1行(`tmpl_line("mov", ..)`)
    pub(in crate::compiler::asm_gen) 
    fn mov_line(
        &self,
        dst: &str,
        src: &str,
        size: &Size,
        is_memory: bool,
    ) -> String {
        self.tmpl_line("mov", dst, src, size, is_memory)
    }
}
