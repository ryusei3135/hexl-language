mod div_ir;
mod insert_fmt_reg;
///! 関数の中身を生成する関数は`src/gen/call_func.rs`にある
mod operand_txt;
mod struct_ir;
mod arr;

use super::*;
use crate::asm_setting;
use crate::compiler::ir::types;

/// 演算結果を格納するレジスタの「適切なサイズ」が、テンプレートを
/// 組み立てる時点ではまだ決まらない演算子の一覧。
///
/// `*`(Mul)/`/`(Div)/`%`(Surplus)は`idiv`/`imul`を使うため、結果を
/// 格納するレジスタのサイズが、オペランドがメモリ上の値かどうかを
/// 見て初めて確定する(`format_expr_inst`内の`check_node_is_memory_value`
/// による判定より後)。そのため、テンプレートを組み立てる時点では
/// レジスタの「番号」だけを埋め込んでおき([`AsmEmitter::replace_insert_fmt_reg`]
/// を参照)、サイズが確定してから改めて実際のレジスタ名へ展開し直す
/// 必要がある。
#[allow(dead_code)]
pub(in crate::compiler::asm_gen) 
const DEFERRED_REG_FMT_OPS: &[inst::ExprKind; 3] = &[
    inst::ExprKind::Mul,
    inst::ExprKind::Div,
    inst::ExprKind::Surplus,
];

/// 式の結果や変数を置くレジスタとして、自動では割り当てないレジスタ。
///
/// - `0`: `%rax` 戻り値・`idiv`の商・システムコール番号
/// - `2`: `%rdx` `idiv`の余り・`cqto`/`cltd`の結果・第3引数
///
/// `/`と`%`は、この2つのレジスタを必ず書き換える。一般の式の結果を
/// ここに置いてしまうと、割り算のたびに壊れてしまう(引数のレジスタが
/// 破壊されるバグの原因の一つだった)ので、割り当て対象から外す。
pub(in crate::compiler::asm_gen) 
const RESERVED_REGS: [usize; 2] = [0, 2];

#[derive(Debug, Clone)]
pub(in crate::compiler::asm_gen) struct VarIndexInfo {
    /// `is_stack`が`false`の場合はレジスタ番号(従来通り)、
    /// `true`の場合は`%rbp`からのバイトオフセットを表す
    pub reg: usize,
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
    pub fn new(reg: usize, size: &types::Size, index: usize) -> Self {
        Self {
            reg: reg,
            size: size.clone(),
            index: index,
            is_stack: false,
            arr_base: 0,
            in_mem: false,
        }
    }

    /// `%rbp`からのオフセット(`offset`)に直接置かれている変数として登録する
    pub fn new_stack(offset: usize, size: &types::Size, index: usize) -> Self {
        Self {
            reg: offset,
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
    pub fn new(asm_setting: asm_setting::AsmSetting, asm_fmt: AsmFmtData) -> Self {
        let mut me = Self {
            asm_text: String::new(),
            data_sec_text: String::new(),
            data_idx: 0,
            reg_idx: 0,
            expr_vars: Vec::new(),
            reserved_label_name: None,

            asm_fmt: asm_fmt::mng_fmt::MngAsmFmt::new(asm_setting, asm_fmt),
            curr_inst: Vec::new(),
            data_map: Vec::new(),
            last_inst_idx: Vec::new(),
            var_hash_map: HashMap::new(),
            used_reg: reg_mnger::UsedRegManager::new(),
            scope_outer_vars: Vec::new(),
            stk_use_counter: 0,
            emitted_calls: Vec::new(),
            arr_index_temp: None,
            struct_stk_map: HashMap::new(),
        };
        me.data_sec_text
            .push_str(&me.asm_fmt.get_section_fmt("data"));
        me
    }

    pub fn to_asm_text(
        &mut self,
        func_tree: &mut def_tree::FuncTree,
        asm_fmt_name: &Option<String>,
        extern_funcs: &[inst::Inst],
        global_funcs: &[String],
    ) -> String {
        // `_start`が定義されているファイルの場合だけ、エントリー
        // ポイントとして`.global _start`を出力する
        // (#includeされるライブラリ用ファイルなど、`_start`を
        //  定義しないファイルにまでこれを出力してしまうと、
        //  存在しないシンボルを公開する不正なアセンブリになる)
        self.asm_text = if func_tree.func.contains_key("_start") {
            String::from(&self.asm_fmt.get_entry_point())
        } else {
            String::new()
        };

        self.gen_global_func_asm(&global_funcs);
        self.gen_extern_func_asm(&extern_funcs);

        // エントリーポイントを先頭に配置
        if let Some(ref mut meta_data) = func_tree.func.remove_entry("_start") {
            self.build_fn_process(meta_data, &asm_fmt_name);
        }

        for mut func_meta_data in func_tree.func.drain() {
            // == アセンブリ言語の生成 ==
            self.build_fn_process(&mut func_meta_data, &asm_fmt_name);
            // == データの初期化 ==
            self.var_hash_map = HashMap::new();
            self.reg_idx = 0;
            self.used_reg.clear();
            self.scope_outer_vars.clear();
        }
        format!(
            "{}\n{}\n{}",
            self.data_sec_text,
            self.asm_fmt.get_section_fmt("text"),
            self.asm_text.replace("{space}", "  ")
        )
    }

    #[inline(always)]
    fn gen_global_func_asm(&mut self, global_funcs: &[String]) {
        // 自身が公開する関数を生成
        for func_name in global_funcs.iter() {
            self.asm_text
                .push_str(self.asm_fmt.get_global_fmt(func_name).as_str());
        }
    }

    #[inline(always)]
    fn gen_extern_func_asm(&mut self, extern_funcs: &[inst::Inst]) {
        for func in extern_funcs.iter() {
            if let inst::Inst::ExternFunc(name) = func {
                self.asm_text
                    .push_str(self.asm_fmt.get_extern_func(&name).as_str());
            } else {
                panic!();
            }
        }
    }

    /// 指定された名前の変数の型を返す
    pub fn get_var_ty(&self, var_name: &str) -> Size {
        self.var_info(var_name).size.clone()
    }

    #[inline(always)]
    pub(super) fn insert_var_info(&mut self, name: &str, var: VarIndexInfo) {
        // メモリに実体がある変数の`reg`はレジスタ番号ではないので、
        // レジスタを使用中にしない
        if !var.in_mem {
            self.used_reg.mark_used(var.reg);
        }
        self.expr_vars.push(name.to_string());
        self.var_hash_map.insert(name.to_owned(), var);
    }

    /// 構造体の実体(`bytes`バイト)を置くスタック領域を`stk_use_counter`から
    /// 確保し、`GetPtr`のノード`ptr_idx`に対応付ける。
    ///
    /// ポインタ(`lea -N(%rbp), %rdi`)は先頭のメンバーを指し、メンバーは
    /// そこからアドレスが小さくなる向きに`(%rdi)`、`-4(%rdi)`のように並ぶ。
    /// 先頭のメンバー自身はポインタより上(アドレスが大きい側)にはみ出すため、
    /// 最大のメンバーのサイズ(8byte)だけ余分に確保する。
    /// `N`は現在のスタック使用量を8byteに切り上げた値に8を足した値とし、
    /// 使用量を`N + bytes`まで進める。これで先にある配列などと重ならない
    pub(in crate::compiler::asm_gen) 
    fn alloc_struct_stk(&mut self, ptr_idx: usize, bytes: usize) -> usize {
        if let Some(offset) = self.struct_stk_map.get(&ptr_idx) {
            return *offset;
        }
        let offset = self.stk_use_counter.div_ceil(8) * 8 + 8;
        self.stk_use_counter = offset + bytes;
        self.struct_stk_map.insert(ptr_idx, offset);
        offset
    }

    /// `GetPtr`のノードの`%rbp`からのオフセット。`alloc_struct_stk`で
    /// 確保済みならそれを、そうでなければIRの`stk`を返す
    pub(in crate::compiler::asm_gen) 
    fn ptr_stk(&self, ptr_idx: usize, ir_stk: usize) -> usize {
        self.struct_stk_map.get(&ptr_idx).copied().unwrap_or(ir_stk)
    }

    /// 式の結果や変数を置く、空いているレジスタを1つ確保して返す。
    ///
    /// 以前は`self.reg_idx`を単純にインクリメントしていたため、
    /// 引数が入っているレジスタ(`%rdi`/`%rsi`など)や、他の変数が
    /// 使っているレジスタまで上書きしていた。ここでは
    /// - 使用中(引数・変数・まだ使う式の結果)のレジスタ
    /// - `RESERVED_REGS`(`%rax`/`%rdx`)
    /// を避けて、最も小さい番号の空きレジスタを返す。
    ///
    /// 返したレジスタを使用中にするのは呼び出し側の責任
    /// (`mark_used`を呼ぶ)。
    pub(in crate::compiler::asm_gen) 
    fn alloc_reg(&self) -> usize {
        (0..self.asm_fmt.reg_count())
            .find(|reg| !RESERVED_REGS.contains(reg) && !self.used_reg.is_used(*reg))
            .expect("使用できるレジスタが足りません")
    }

    /// `node_idx`が式(`Inst::Expr`)の場合、その結果を持っていた一時的な
    /// レジスタを解放する。親の式や代入が、その値を使い終わった時点で
    /// 呼ぶ。
    ///
    /// 変数がそのレジスタを持っている場合は解放しない。
    pub(super) fn release_expr_temp(&mut self, node_idx: usize) {
        if !matches!(self.curr_inst[node_idx], inst::Inst::Expr(..)) {
            return;
        }
        let Some(reg) = self
            .last_inst_idx
            .iter()
            .find(|(id, _)| *id == node_idx)
            .map(|(_, reg)| *reg)
        else {
            return;
        };
        if !self.is_reg_held_by_var(reg) {
            self.used_reg.release(reg);
        }
    }

    /// 現在`reg`を使っている値を、`push`で退避するためのレジスタ名の一覧
    /// (`dst`は除く)。`/`や`%`が書き換える`%rax`/`%rdx`のうち、
    /// 使用中のものを退避するために使う。
    pub(super) fn live_reserved_regs(&self, dst: usize) -> Vec<usize> {
        RESERVED_REGS
            .iter()
            .copied()
            .filter(|reg| *reg != dst && self.used_reg.is_used(*reg))
            .collect()
    }

    /// 関数一つ分のアセンブリ言語を生成し始める前に、前の関数の
    /// 状態を全て捨てる。
    ///
    /// 以前は`_start`を最初に生成した後に、この初期化をしていなかった
    /// ため、`_start`の変数(`var_hash_map`)や使用中のレジスタ、
    /// 式の結果のレジスタ(`last_inst_idx`)が次の関数へ持ち越されていた。
    /// (`last_inst_idx`は関数ごとに振り直される式のidで引くので、
    ///  前の関数の同じidの式のレジスタを誤って使うことがあった)
    pub(in crate::compiler::asm_gen) 
    fn reset_fn_state(&mut self) {
        self.var_hash_map = HashMap::new();
        self.struct_stk_map.clear();
        self.reg_idx = 0;
        self.used_reg.clear();
        self.scope_outer_vars.clear();
        self.expr_vars.clear();
        self.last_inst_idx.clear();
        self.reserved_label_name = None;
    }

    /// `Inst::StartScope`の処理
    /// これ以降に登録されたレジスタを記録し始める
    pub(in crate::compiler::asm_gen) 
    fn start_scope_asm(&mut self) {
        self.used_reg.start_scope();
        self.scope_outer_vars
            .push(self.var_hash_map.keys().cloned().collect());
    }

    /// `Inst::EndScope`の処理
    /// `StartScope`以降に登録されたレジスタを全て解放する。
    ///
    /// 解放したレジスタのうち、空いている最小の番号を`self.reg_idx`に
    /// 設定するので、スコープ内で使ったレジスタを次の式が再利用できる。
    ///
    /// ただし、スコープの外で宣言された変数が(スコープ内での再代入などで)
    /// レジスタを使うようになった場合、そのレジスタはスコープが終わっても
    /// 生きているので、解放せずに残す
    pub(in crate::compiler::asm_gen) 
    fn end_scope_asm(&mut self) {
        let released = self.used_reg.end_scope();
        let outer_vars = self
            .scope_outer_vars
            .pop()
            .expect("StartScopeに対応しないEndScopeです");
        let live_regs: Vec<usize> = outer_vars
            .iter()
            .filter_map(|name| self.var_reg(name))
            .collect();
        self.used_reg.restore(&live_regs);

        // 次の式の結果を置くレジスタ(`reg_idx`)を、解放したレジスタに戻す。
        // 外側の変数が使っているレジスタ(上で登録し直したもの)は
        // 除き、`reg_idx`より小さい番号に限る(番号を進めることはしない)
        if let Some(reg) = self.used_reg.lowest_free(&released) {
            if reg < self.reg_idx {
                self.reg_idx = reg;
            }
        }
    }

    #[inline(always)]
    pub(in crate::compiler::asm_gen) 
    fn update_value_info(&mut self, name: &str, index: usize) {
        self.var_info_mut(name).index = index;
    }

    #[inline(always)]
    pub(in crate::compiler::asm_gen) 
    fn update_value_reg(&mut self, name: &str, reg: usize) {
        self.used_reg.mark_used(reg);
        self.var_info_mut(name).reg = reg;
    }

    /// 渡された情報を、設定したアセンブリ言語のフォーマット
    /// 通りに加工する。
    pub(in crate::compiler::asm_gen) 
    fn format_line(
        &mut self,
        opcode: &str,
        dst: Option<usize>,
        src1: usize,
        src2: Option<usize>,
        this_is_self: &SelfPtrInfo,
    ) -> String {
        let base_size: Size = self.check_node_is_mem_val(src1).unwrap_or(Size::DQ);
        let mut formated = if let Some(struct_idx) = self.resolve_struct_idx(src1) {
            // 構造体の生成
            let mut txt = self.extract_operand_text(struct_idx, &this_is_self);

            let ret_line = if this_is_self.is_none() {
                let self_ptr_reg = self.self_ptr_reg();
                let line = self.fill_tmpl("address", &self.get_reg(dst, &Size::DQ), &self_ptr_reg);
                self.asm_fmt.fmt_mnemonic_resize("lea", &line, &Size::DQ)
            } else {
                self.fill_tmpl(opcode, &self.get_reg(dst, &base_size), FRAME_BASE_REG)
            };
            txt.push_str(&ret_line);
            txt
        } else {
            let dst_size = self.resolve_dst_reg_size(opcode, src1, this_is_self);
            let dst_text = self.get_reg(dst, &dst_size);
            let src_text = self.extract_operand_text(src1, &this_is_self);
            // srcがレジスタの場合は、dstとサイズを揃える
            // (`movl %rcx, %edx`のような、サイズの混在を防ぐ)
            let src_text = if opcode == "address" {
                src_text
            } else {
                self.asm_fmt.resize_reg_operand(&src_text, &dst_size)
            };
            self.fill_tmpl(opcode, &dst_text, &src_text)
        };

        if let Some(resized_asm) = self.gen_resize_mnemonic(&formated, opcode, src1) {
            formated = resized_asm;
        }

        if let Some(src2_id) = src2 {
            formated.replace("{src2}", &self.extract_operand_text(src2_id, &this_is_self))
        } else {
            formated
        }
    }

    /// レジスタに載せる値の型`ty`から、実際に使うレジスタの
    /// サイズを決める。
    ///
    /// - `DB`/`DW`/`DD`/`DQ`: そのままのサイズ
    /// - 配列: 要素のサイズ
    /// - ポインタ/構造体など: アドレスを持つので64bit(`DQ`)
    pub(in crate::compiler::asm_gen) 
    fn value_reg_size(ty: &Size) -> Size {
        match ty {
            Size::DB | Size::DW | Size::DD | Size::DQ => ty.clone(),
            Size::Array { size, .. } => Self::value_reg_size(size),
            _ => Size::DQ,
        }
    }

    /// `format_line`で、結果を書き込む先(`dst`)のレジスタの
    /// サイズを決める。
    ///
    /// 1. `address`(`lea`)は常にアドレスなので64bit
    /// 2. メモリ上の値を読む場合は、読む対象のサイズ
    /// 3. 即値・式の結果・関数の戻り値など(メモリ以外)は、
    ///    代入先の型(`dst_ty`)のサイズ
    /// 4. `dst_ty`が無い(`self`を持つメソッド内など)場合は、
    ///    値そのものの型から推測できるときだけその型、
    ///    できなければ64bit
    ///
    /// 以前は3.と4.が無く、メモリ以外の値は常に64bitレジスタ
    /// (`%rcx`など)に書き込まれていた。
    fn resolve_dst_reg_size(&self, opcode: &str, src1: usize, dst_ty: &SelfPtrInfo) -> Size {
        if opcode == "address" {
            return Size::DQ;
        }
        if let Some(size) = self.check_node_is_mem_val(src1) {
            return Self::value_reg_size(&size);
        }
        if let Some(ty) = dst_ty {
            return Self::value_reg_size(ty);
        }
        match &self.curr_inst[src1] {
            inst::Inst::Num { size, .. } => Self::value_reg_size(size),
            inst::Inst::Expr(..) => Self::value_reg_size(&self.get_expr_ty(src1)),
            _ => Size::DQ,
        }
    }

    #[inline(always)]
    fn gen_resize_mnemonic(&self, formated: &str, opcode: &str, src1: usize) -> Option<String> {
        let mut resize = self.check_node_is_mem_val(src1).unwrap_or(Size::DQ);
        let mnemonic: &str = if opcode == "address" {
            resize = Size::DQ;
            "lea"
        } else if self.check_node_is_struct(src1) {
            "mov"
        } else if let Some(size) = self.check_node_is_mem_val(src1) {
            resize = size;
            "mov"
        } else {
            return None;
        };
        let resized = if self.check_node_is_mem_val(src1).is_some() {
            self.asm_fmt
                .fmt_memory_mnemonic_resize(mnemonic, &formated, &resize)
        } else {
            self.asm_fmt
                .fmt_mnemonic_resize(mnemonic, formated, &resize)
        };
        Some(resized)
    }

    /// IRの値IDから、その式が生成する値の型をたどって取得する。
    pub(super) 
    fn get_expr_ty(&self, node_idx: usize) -> Size {
        match self.curr_inst[node_idx] {
            inst::Inst::Expr(ref expr) => self.get_expr_ty(expr.ls),
            inst::Inst::Pointer(inner) => match self.get_expr_ty(inner) {
                Size::Pointer { ty, .. } => *ty,
                ty => ty,
            },
            inst::Inst::GetAddress(..) => Size::DQ,
            inst::Inst::InsertArr { ref name, .. } => {
                self.arr_elem_ty(name)
            }
            inst::Inst::AssignVar { value, .. } => self.get_expr_ty(value),
            inst::Inst::InitArr(ref ids) => {
                let size = ids
                    .first()
                    .map(|id| self.get_expr_ty(*id))
                    .unwrap_or(Size::Void);
                Size::Array {
                    size: Box::new(size),
                    len: ids.len(),
                }
            }
            inst::Inst::CallFunc(..) => Size::DQ,
            ref node => node
                .get_param_ty()
                .unwrap_or_else(|| panic!("cannot determine expression type: {:?}", node)),
        }
    }

    /// `idx`が(直接、あるいは`GetAddress`/`Pointer`でラップされた先に)
    /// `Inst::Struct`を指している場合、その`Inst::Struct`自身のidxを返す。
    /// `format_line`が構造体の生成を特別扱いする際、ラップされた
    /// ノードの内側までたどれるようにするためのヘルパー
    pub(in crate::compiler::asm_gen) 
    fn resolve_struct_idx(&self, idx: usize) -> Option<usize> {
        match self.curr_inst[idx] {
            inst::Inst::Struct { .. } => Some(idx),
            inst::Inst::GetAddress(inner)
            | inst::Inst::Pointer(inner)
            | inst::Inst::Mov { src: inner, .. } => self.resolve_struct_idx(inner),
            _ => None,
        }
    }

    fn check_node_is_struct(&self, node_idx: usize) -> bool {
        match &self.curr_inst[node_idx] {
            inst::Inst::RefStruct { .. } => true,
            _ => false,
        }
    }

    /// 渡されたノードがスタック/静的領域の変数(`MemoryValue`)を
    /// 参照している場合、そのサイズを返す。
    /// これは`mov`命令に付けるサイズ接尾辞(`movl`など)を
    /// 決定するために使う。
    #[inline(always)]
    pub(in crate::compiler::asm_gen) 
    fn check_node_is_mem_val(&self, node_idx: usize) -> Option<Size> {
        self.check_node_is_mem_val_inner(node_idx, false)
    }

    /// `check_node_is_mem_val`の実装本体
    /// ## 引数
    /// - is_deref
    ///     現在たどっている経路が`Inst::Pointer`(`*ptr`のような
    ///     参照先の読み書き)を経由しているかどうか。
    ///     `Inst::MemoryValue`にたどり着いたとき、これが`true`なら
    ///     ポインタが指す「参照先」の型のサイズ(`ty`)を、`false`なら
    ///     ポインタ変数「自身」の値のサイズ(常に8byte)を返す。
    fn check_node_is_mem_val_inner(&self, node_idx: usize, is_deref: bool) -> Option<Size> {
        match self.curr_inst[node_idx] {
            inst::Inst::MemoryValue(inst::MemoryInst::Memory { ref size, .. }) => match size {
                // `*ptr`のように参照先を読み書きする場合のみ、
                // 参照先の型`ty`のサイズを使う
                Size::Pointer { ty, .. } if is_deref => Some(*(*ty).clone()),
                // それ以外(ポインタ変数自身の値をそのまま読む場合)は、
                // ポインタの実際のサイズ(常に8byte)を使う
                Size::Pointer { .. } => Some(Size::DQ),
                other => Some(other.clone()),
            },
            inst::Inst::Pointer(inner) => {
                // ここから先は「参照先」をたどる経路になる
                self.check_node_is_mem_val_inner(inner, true)
            }
            inst::Inst::GetAddress(inner) => {
                // アドレスを求める経路では参照先をたどっているわけ
                // ではないので、`is_deref`はそのまま引き継ぐ
                self.check_node_is_mem_val_inner(inner, is_deref)
            }
            inst::Inst::InsertArr { ref name, .. } => self.var_size(name),
            _ => None,
        }
    }

    /// ## 引数
    /// - reg_idx これは必ずusizeで無ければいけない、
    fn get_reg(&self, reg_idx: Option<usize>, size: &Size) -> String {
        let num = if reg_idx.is_none() {
            self.reg_idx
        } else {
            reg_idx.unwrap()
        };
        self.asm_fmt.get_fmt_reg(num, &size)
    }

    pub(in crate::compiler::asm_gen) 
    fn extract_operand_text(
        &mut self,
        parent_id: usize,
        this_is_self: &Option<types::Size>,
    ) -> String {
        match self.inst_at(parent_id) {
            inst::Inst::Num { value, .. } => self.asm_fmt.get_fmt_num(&value),
            inst::Inst::GetPtr { size: _, stk } => {
                // スタック上に置かれた値そのもの(値が置かれているメモリ)
                // を指すオペランドを、`%rbp`からのオフセットを使って生成する
                // (構造体の実体は`alloc_struct_stk`で確保したオフセットを使う)
                let stk = self.ptr_stk(parent_id, stk);
                self.rbp_ref(stk)
            }
            inst::Inst::Param(param) => {
                // `asm_emitter/operand_txt/`に記述
                self.param_ref(&param.name)
            }
            inst::Inst::AssignVar { ref name, .. } => self.var_operand(name),
            // 配列にアクセス
            inst::Inst::InsertArr { name, dst, index } => {
                // `asm_emitter/operand_txt/`に記述
                self.insert_arr_txt(&name, dst, index, this_is_self)
            }
            inst::Inst::Str { .. } => {
                // `asm_emitter/operand_txt/`に記述
                self.string_mem_ref(parent_id)
            }
            inst::Inst::Mov { ref name, src, .. } => {
                // `asm_emitter/operand_txt/`に記述
                self.gen_mov_code(name.as_deref(), src)
            }
            inst::Inst::Block(name) => name.to_string(),
            inst::Inst::ExpectJmp(name) => name.to_string(),
            inst::Inst::Struct { mem, .. } => {
                self.emit_struct_ini_asm(
                    mem,
                    // Noneの場合それはSelf
                    this_is_self.is_none(),
                )
            }
            inst::Inst::MemoryValue(inst::MemoryInst::Memory { kind, size, .. }) => {
                // `asm_emitter/operand_txt/`に記述
                self.ref_mem_value_txt(&kind, &size, parent_id)
            }
            inst::Inst::RefStruct { src, pos, size } => {
                // `asm_emitter/operand_txt/`に記述
                self.ref_struct_txt(&src, pos, size.to_bytes())
            }
            inst::Inst::GetAddress(index) => self.extract_operand_text(index, this_is_self),
            // 配列リテラル自体を値として参照する場合
            // (例: 変数に束縛されずそのまま関数の引数などに使われる`{1,2,3}`)
            inst::Inst::InitArr(ids) => {
                // `asm_emitter/operand_txt/`に記述
                self.init_arr_txt::<false>(&ids, this_is_self);
                String::new()
            }
            inst::Inst::CallFunc(call_fn_info) => {
                // 関数呼び出しのアセンブリが`build_fn_process`側で
                // 既に生成済みか
                let already_emitted = self.emitted_calls.contains(&parent_id);

                if !already_emitted && call_fn_info.parent != crate::ir::IS_ASSIGN_EXPR {
                    // 呼び出しを生成する対象ではない
                    String::new()
                } else {
                    if !already_emitted {
                        let call_asm = self.emit_call_func(
                            &call_fn_info,
                            matches!(this_is_self, Some(types::Size::Struct(..))),
                        );
                        self.asm_text.push_str(&call_asm);
                    }
                    // 戻り値のレジスタ(0番、`%eax`など)を返す。
                    // 戻り値を受ける側の型(`this_is_self`)がある
                    // 場合はそのサイズ、なければ64bit(`%rax`)
                    let ret_size = this_is_self
                        .as_ref()
                        .map(Self::value_reg_size)
                        .unwrap_or(Size::DQ);
                    self.asm_fmt.get_fmt_reg(0, &ret_size)
                }
            }
            inst::Inst::Pointer(index) => self.extract_operand_text(index, this_is_self),
            t => {
                if let Some(result) = self.last_inst_idx.iter().find(|i| i.0 == parent_id) {
                    // 式の結果を持つレジスタは、その式自身の型のサイズで
                    // 取得する(常に64bitにすると、`movl %rcx, %edx`の
                    // ように32bitの命令へ64bitのレジスタが混ざる)
                    let size = match &t {
                        inst::Inst::Expr(..) => Self::value_reg_size(&self.get_expr_ty(parent_id)),
                        _ => Size::DQ,
                    };
                    self.asm_fmt.get_fmt_reg(result.1, &size)
                } else {
                    panic!("{:?}", t);
                }
            }
        }
    }

    pub(super) fn format_expr_inst(&mut self, expr: &inst::ExprInst) -> String {
        // `/`と`%`は`%rax`/`%rdx`を使う専用の手順で生成する
        // (`div_ir.rs`)。テンプレートでは、割られる数を`%rax`ではなく
        // `%rdx`に置いてしまい、さらに`%rdx`にある引数を壊していた
        if matches!(
            expr.kind,
            inst::ExprKind::Div | inst::ExprKind::Surplus
        ) {
            return self.format_div_expr_inst(expr);
        }

        let key = match expr.kind {
            inst::ExprKind::Add => "add",
            inst::ExprKind::Sub => "sub",
            // 乗算は`add`のテンプレートの命令を`imul`に置き換えて使う
            // (2オペランドの`imul`は`%rax`/`%rdx`を書き換えない)
            inst::ExprKind::Mul => "add",
            inst::ExprKind::Div | inst::ExprKind::Surplus => unreachable!(),
            inst::ExprKind::LessThen => "cmp_l",
            inst::ExprKind::GreaterThen => "cmp_g",
            inst::ExprKind::Equal => "cmp_e",
            inst::ExprKind::NotEq => "cmp_ne",
        };
        // ニーモニックのサイズ調整に使う、実際のニーモニックの文字列
        // (`cmp_l`/`cmp_g`はテンプレートを引くためのキーであって、
        //  実際にテンプレート中で使われるニーモニックは"cmp"なので、
        //  `fmt_mnemonic_resize`にはそちらを渡す必要がある)
        let mnemonic = match expr.kind {
            inst::ExprKind::Add => "add",
            inst::ExprKind::Sub => "sub",
            inst::ExprKind::Mul => "imul",
            inst::ExprKind::Div | inst::ExprKind::Surplus => unreachable!(),
            inst::ExprKind::LessThen
            | inst::ExprKind::GreaterThen
            | inst::ExprKind::NotEq
            | inst::ExprKind::Equal => "cmp",
        };

        let resolved_size: Size = self.get_expr_ty(expr.ls);
        let wrap_size = resolved_size.wrap_dst_size();

        let dst_text = self.get_reg(Some(self.reg_idx), &resolved_size);

        let mut tmpl = self.asm_fmt.get_opcode_tmpl(key);
        if expr.kind == inst::ExprKind::Mul {
            tmpl = tmpl.replace("add", "imul");
        }

        let mut formated = tmpl
            .replace("{dst}", &dst_text)
            .replace("{src1}", &self.extract_operand_text(expr.ls, &wrap_size))
            .replace("{src2}", &self.extract_operand_text(expr.rs, &wrap_size))
            .to_string();

        let is_memory_access = self.check_node_is_mem_val(expr.ls).is_some()
            || self.check_node_is_mem_val(expr.rs).is_some();
        formated = self.fmt_one_expr_mnemo_resize(
            formated.to_string(),
            &resolved_size,
            mnemonic,
            is_memory_access,
        );

        // サイズがSelfでない場合
        if self.reserved_label_name.is_some() && formated.contains("{label}") {
            let name = self.reserved_label_name.take().unwrap();
            formated.replace("{label}", &name)
        } else {
            formated
        }
    }
}
