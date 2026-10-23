mod arr;
mod cast;
mod data_mem_factory;
mod div_ir;
mod emit_tmpl_key;
mod insert_fmt_reg;
///! 関数の中身を生成する関数は`src/gen/call_func.rs`にある
mod operand_txt;
mod set_cc;
mod ternary;
mod var_mnger;

use super::emit_fn_name::emit_fn_name_id;
use super::*;
use crate::asm_setting;
use crate::compiler::err::{self, err_pool};
use crate::compiler::ir::inst::Inst;
use crate::compiler::ir::types;

use super::*;

pub use var_mnger::VarIndexInfo;

impl AsmEmitter {
    pub fn new(
        asm_setting: asm_setting::AsmSetting,
        asm_fmt: AsmFmtData,
        os: &str,
        err_pool: &Rc<RefCell<err_pool::ErrPool>>,
    ) -> Self {
        let caller = Rc::new(vec![1, 4, 5, 6, 7, 8, 9]);
        let callee = Rc::new(vec![3, 10, 11, 12, 13]);
        let float_caller = Rc::new((0..16).collect());
        let float_callee = Rc::new(Vec::new());

        let mut me = Self {
            asm_text: String::new(),
            data_sec_text: String::new(),
            data_idx: 0,
            reg_idx: 0,
            expr_vars: Vec::new(),
            reserved_label_name: None,

            asm_fmt: asm_fmt::mng_fmt::MngAsmFmt::new(asm_setting, asm_fmt, os),
            os: os.to_string(),

            curr_inst: Vec::new(),
            data_map: Vec::new(),
            last_inst_idx: Vec::new(),
            var_hash_map: HashMap::new(),
            used_reg: reg_mnger::UsedRegManager::new(
                Rc::clone(&callee),
                Rc::clone(&caller),
                Rc::clone(&float_callee),
                Rc::clone(&float_caller),
            ),
            scope_outer_vars: Vec::new(),
            stk_use_counter: 0,
            emitted_calls: Vec::new(),
            unused_var_nodes: std::collections::HashSet::new(),
            pending_funcs: HashMap::new(),
            fn_asm_out: String::new(),
            asm_fmt_name: None,
            arr_index_temp: None,
            struct_stk_map: HashMap::new(),
            spilled_regs: std::collections::BTreeMap::new(),
            spill_slots: std::collections::BTreeMap::new(),
            node_vars: std::collections::HashSet::new(),
            node_vars_all: false,
            dead_vars: std::collections::HashSet::new(),

            callee_saved: Rc::clone(&callee),
            caller_saved: Rc::clone(&caller),

            float_callee_saved: Rc::clone(&float_callee),
            float_caller_saved: Rc::clone(&float_caller),

            err_pool: Rc::clone(&err_pool),
        };
        me.data_sec_text
            .push_str(&me.asm_fmt.get_section_fmt("data"));
        me
    }

    pub fn to_asm_text(
        &mut self,
        mut func_tree: def_tree::FuncTree,
        asm_fmt_name: Option<&str>,
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

        let mut start_func = func_tree.func.remove_entry("_start");

        self.gen_global_func_asm(&global_funcs);
        self.gen_extern_func_asm(&extern_funcs);

        // `_start`以外の関数は、`call_func.rs`の`emit_call_func`が
        // 呼び出しを生成するときに`build_fn_process`で生成する
        // (呼ばれない関数は生成しない)。ここでは`pending_funcs`へ
        // 登録するだけで、生成するのは次の関数のみ。
        //  - `_start`(エントリーポイント)
        //  - `global_funcs`(他のファイルから呼ばれるので、起点として生成する)
        self.pending_funcs.clear();
        self.fn_asm_out.clear();
        self.asm_fmt_name = asm_fmt_name.map(str::to_string);
        for (name, info) in func_tree.func.drain() {
            if name == "_start" {
                continue;
            }
            let label = Self::fn_label_of(&info.name, &info.temp_ty);
            self.pending_funcs.insert(label, (name, info));
        }

        // エントリーポイントを先頭に配置
        if let Some(ref mut meta_data) = start_func {
            self.build_fn_process(meta_data, asm_fmt_name);
        }

        // 公開する関数(未生成のもの)
        for fn_name in global_funcs {
            let label = emit_fn_name_id(fn_name);
            if let Some(mut meta_data) = self.pending_funcs.remove(&label) {
                self.emit_callee_fn(&mut meta_data);
            }
        }
        // 呼ばれた側の関数のアセンブリを、`_start`の後ろへ続ける
        let callee_asm = mem::take(&mut self.fn_asm_out);
        self.asm_text.push_str(&callee_asm);
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
        global_funcs.iter().for_each(|fn_name| {
            let label = emit_fn_name_id(fn_name);
            self.asm_text
                .push_str(self.asm_fmt.get_global_fmt(&label).as_str());
        });
    }

    #[inline(always)]
    fn gen_extern_func_asm(&mut self, extern_funcs: &[inst::Inst]) {
        extern_funcs.iter().for_each(|func| {
            if let inst::Inst::ExternFunc(name) = func {
                let label = emit_fn_name_id(name);
                self.asm_text
                    .push_str(self.asm_fmt.get_extern_func(&label).as_str());
            } else {
                println!("warning");
            }
        });
    }

    #[inline(always)]
    pub(super) fn insert_var_info(&mut self, name: &str, var: VarIndexInfo) {
        // メモリに実体がある変数(`in_mem`)と、`%rbp`相対で置かれた
        // 変数(`is_stack`)はレジスタを持たない(`offset`で保存する)ので、
        // レジスタを使用中にしない
        if !var.in_mem && !var.is_stack {
            self.used_reg.mark_used(&var.reg);
        }
        self.expr_vars.push(name.to_string());
        self.dead_vars.remove(name);
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
    pub(in crate::compiler::asm_gen) fn alloc_struct_stk(
        &mut self,
        ptr_idx: usize,
        bytes: usize,
    ) -> usize {
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
    pub(in crate::compiler::asm_gen) fn ptr_stk(&self, ptr_idx: usize, ir_stk: usize) -> usize {
        self.struct_stk_map.get(&ptr_idx).copied().unwrap_or(ir_stk)
    }

    /// `node_idx`が式(`Inst::Expr`)の場合、その結果を持っていた一時的な
    /// レジスタを解放する。親の式や代入が、その値を使い終わった時点で
    /// 呼ぶ。
    ///
    /// 変数がそのレジスタを持っている場合は解放しない。
    pub(super) fn release_expr_temp(&mut self, node_idx: usize) {
        if !matches!(
            self.curr_inst[node_idx],
            inst::Inst::Expr(..) | inst::Inst::Cast { .. }
        ) {
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
        if self.is_reg_held_by_var(reg) {
            return;
        }

        self.used_reg.release(&reg);
    }

    /// 現在`reg`を使っている値を、`push`で退避するためのレジスタ名の一覧
    /// (`dst`は除く)。`/`や`%`が書き換える`%rax`/`%rdx`のうち、
    /// 使用中のものを退避するために使う。
    pub(super) fn live_reserved_regs(&self, dst: usize) -> Vec<usize> {
        RESERVED_REGS
            .iter()
            .copied()
            .filter(|reg| *reg != dst && self.used_reg.is_used(&RegisterMode::Normal(*reg)))
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
    pub(in crate::compiler::asm_gen) fn reset_fn_state(&mut self) {
        self.var_hash_map = HashMap::new();
        self.struct_stk_map.clear();
        self.spilled_regs.clear();
        self.spill_slots.clear();
        self.node_vars.clear();
        self.node_vars_all = false;
        self.dead_vars.clear();
        self.reg_idx = 0;
        self.used_reg.clear();
        self.scope_outer_vars.clear();
        self.expr_vars.clear();
        self.last_inst_idx.clear();
        self.reserved_label_name = None;
        // `data_map`は「IRのノードid → 静的領域のラベル」の対応表で、
        // ノードidは関数ごとに振り直される。前の関数の分が残っていると、
        // `static_label`が先頭から探すため、別の関数の同じidのラベル
        // (`M1`など)を誤って返してしまう。ラベル名の重複は`data_idx`
        // (関数をまたいで増え続ける)で防いでおり、データ領域の
        // テキスト(`data_sec_text`)は残すので、表だけを捨てる
        self.data_map.clear();
    }

    /// `Inst::StartScope`の処理
    /// これ以降に登録されたレジスタを記録し始める
    pub(in crate::compiler::asm_gen) fn start_scope_asm(&mut self) {
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
    pub(in crate::compiler::asm_gen) fn end_scope_asm(&mut self) {
        let released = self.used_reg.end_scope();
        let outer_vars = self
            .scope_outer_vars
            .pop()
            .expect("StartScopeに対応しないEndScopeです");
        // スコープの中で定義した変数は、もう使われない
        let dead: Vec<String> = self
            .var_hash_map
            .keys()
            .filter(|name| !outer_vars.contains(name))
            .cloned()
            .collect();
        self.dead_vars.extend(dead);
        let live_regs: Vec<RegisterMode> = outer_vars
            .iter()
            .filter_map(|name| self.var_reg(name))
            .collect();
        self.used_reg.restore(&live_regs);
        // 解放されたレジスタの退避は、復元せずに捨てる
        self.drop_dead_spills();

        // 次の式の結果を置くレジスタ(`reg_idx`)を、解放したレジスタに戻す。
        // 外側の変数が使っているレジスタ(上で登録し直したもの)は
        // 除き、`reg_idx`より小さい番号に限る(番号を進めることはしない)
        if let Some(RegisterMode::Normal(reg)) = self.used_reg.lowest_free(&released) {
            if *reg < self.reg_idx {
                self.reg_idx = *reg;
            }
        }
    }

    /// 渡された情報を、設定したアセンブリ言語のフォーマット
    /// 通りに加工する。
    pub(in crate::compiler::asm_gen) fn format_line(
        &mut self,
        opcode: &str,
        dst: Option<usize>,
        src1: usize,
        src2: Option<usize>,
        this_is_self: SelfPtrInfo,
    ) -> Result<String, err::ErrKind> {
        let mut formated = if let Some(variant_idx) = self.resolve_variant_idx(src1) {
            self.format_variant_init_line(variant_idx, dst, this_is_self)?
        } else if let Some(struct_idx) = self.resolve_struct_idx(src1) {
            self.format_struct_init_line(opcode, struct_idx, dst, src1, this_is_self)?
        } else {
            self.format_value_line(opcode, dst, src1, this_is_self)?
        };

        if let Some(resized_asm) = self.gen_resize_mnemonic(&formated, opcode, src1)? {
            formated = resized_asm;
        }

        Ok(if let Some(src2_id) = src2 {
            formated.replace("{src2}", &self.extract_operand_text(src2_id, this_is_self)?)
        } else {
            formated
        })
    }

    /// バリアントの生成: 初期化の命令を出した後、先頭(タグ)の
    /// アドレスをdstへ渡す(構造体と同じ扱い)
    fn format_variant_init_line(
        &mut self,
        variant_idx: usize,
        dst: Option<usize>,
        this_is_self: SelfPtrInfo,
    ) -> Result<String, err::ErrKind> {
        let mut txt = self.extract_operand_text(variant_idx, this_is_self)?;
        let is_self = matches!(
            self.curr_inst[variant_idx],
            inst::Inst::Variant { is_self: true, .. }
        );
        if !is_self {
            let stk = self.ptr_stk(variant_idx, 0);
            let src = self.rbp_ref(stk);
            let dst_text = self.get_reg_txt(dst, &Size::DQ).unwrap();
            let line = self.fill_tmpl("address", &dst_text, &src);
            txt.push_str(&self.asm_fmt.fmt_mnemonic_resize("lea", &line, &Size::DQ));
        }
        Ok(txt)
    }

    /// 構造体の生成: 初期化の命令を出した後、構造体のアドレス
    /// (`self`の中なら暗黙の`self`ポインタ)をdstへ渡す
    fn format_struct_init_line(
        &mut self,
        opcode: &str,
        struct_idx: usize,
        dst: Option<usize>,
        src1: usize,
        this_is_self: SelfPtrInfo,
    ) -> Result<String, err::ErrKind> {
        let base_size: Size = self.check_node_is_mem_val(src1).unwrap_or(Size::DQ);
        let mut txt = self.extract_operand_text(struct_idx, this_is_self)?;

        let ret_line = if this_is_self.is_none() {
            let self_ptr_reg = self.self_ptr_reg();
            let line = self.fill_tmpl("address", &self.get_reg_txt(dst, &Size::DQ)?, &self_ptr_reg);
            self.asm_fmt.fmt_mnemonic_resize("lea", &line, &Size::DQ)
        } else {
            self.fill_tmpl(opcode, &self.get_reg_txt(dst, &base_size)?, FRAME_BASE_REG)
        };
        txt.push_str(&ret_line);
        Ok(txt)
    }

    /// 普通の値(数値・変数・式の結果など)をdstへ渡す命令を作る
    fn format_value_line(
        &mut self,
        opcode: &str,
        dst: Option<usize>,
        src1: usize,
        this_is_self: SelfPtrInfo,
    ) -> Result<String, err::ErrKind> {
        let dst_size = self.resolve_dst_reg_size(opcode, src1, this_is_self);
        let dst_text = self.get_reg_txt(dst, &dst_size)?;
        let src_text = self.extract_operand_text(src1, this_is_self)?;
        // srcがレジスタの場合は、dstとサイズを揃える
        // (`movl %rcx, %edx`のような、サイズの混在を防ぐ)
        let src_text = if opcode == "address" {
            src_text
        } else {
            self.asm_fmt.resize_reg_operand(&src_text, &dst_size)?
        };
        Ok(self.fill_tmpl(opcode, &dst_text, &src_text))
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
    pub(super) fn get_expr_ty(&self, node_idx: usize) -> Size {
        match self.curr_inst[node_idx] {
            inst::Inst::Expr(ref expr) => self.get_expr_ty(expr.ls),
            inst::Inst::Pointer(inner) => match self.get_expr_ty(inner) {
                Size::Pointer { ty, .. } => *ty,
                ty => ty,
            },
            inst::Inst::GetAddress(..) => Size::DQ,
            inst::Inst::InsertArr { ref name, .. } => self.arr_elem_ty(name),
            inst::Inst::Cast { ref after, .. } => after.clone(),
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
    pub(in crate::compiler::asm_gen) fn resolve_struct_idx(&self, idx: usize) -> Option<usize> {
        match self.curr_inst[idx] {
            inst::Inst::Struct { .. } => Some(idx),
            inst::Inst::GetAddress(inner)
            | inst::Inst::Pointer(inner)
            | inst::Inst::Mov { src: inner, .. } => self.resolve_struct_idx(inner),
            _ => None,
        }
    }

    /// `resolve_struct_idx`のバリアント版
    fn resolve_variant_idx(&self, idx: usize) -> Option<usize> {
        match self.curr_inst[idx] {
            inst::Inst::Variant { .. } => Some(idx),
            inst::Inst::GetAddress(inner)
            | inst::Inst::Pointer(inner)
            | inst::Inst::Mov { src: inner, .. } => self.resolve_variant_idx(inner),
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
    pub(in crate::compiler::asm_gen) fn check_node_is_mem_val(
        &self,
        node_idx: usize,
    ) -> Option<Size> {
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

    pub(in crate::compiler::asm_gen) fn extract_operand_text(
        &mut self,
        parent_id: usize,
        this_is_self: SelfPtrInfo,
    ) -> Result<String, err::ErrKind> {
        match self.inst_at(parent_id) {
            inst::Inst::Num { value, .. } => Ok(self.asm_fmt.get_fmt_num(&value)),
            inst::Inst::Float { .. } => {
                let label = self
                    .static_label(parent_id)
                    .expect("float literal data label not found");
                Ok(self.asm_fmt.fmt_static_var_rip(&label))
            }
            inst::Inst::GetPtr { size: _, stk } => {
                // スタック上に置かれた値そのもの(値が置かれているメモリ)
                // を指すオペランドを、`%rbp`からのオフセットを使って生成する
                // (構造体の実体は`alloc_struct_stk`で確保したオフセットを使う)
                let stk = self.ptr_stk(parent_id, stk);
                Ok(self.rbp_ref(stk))
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
            inst::Inst::Block(name) => Ok(name.to_string()),
            inst::Inst::ExpectJmp(name) => Ok(name.to_string()),
            inst::Inst::Struct { mem, .. } => {
                self.emit_struct_init_asm(
                    mem,
                    // Noneの場合それはSelf
                    this_is_self.is_none(),
                )
            }
            inst::Inst::Variant {
                tag,
                value,
                size,
                is_self,
                ..
            } => self.emit_data_variant_init_asm(parent_id, value.as_ref(), tag, is_self, &size),
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
                Ok(String::new())
            }
            inst::Inst::CallFunc(call_fn_info) => {
                self.call_result_operand(parent_id, &call_fn_info, this_is_self)
            }
            inst::Inst::Pointer(index) => self.extract_operand_text(index, this_is_self),
            t => self.expr_result_operand(parent_id, &t),
        }
    }

    /// 関数呼び出しのノード`parent_id`を値として使う際のオペランドを返す
    ///
    /// まだ呼び出しのアセンブリが生成されていなければここで生成し、
    /// 戻り値のレジスタ(0番、`%eax`など)を返す
    fn call_result_operand(
        &mut self,
        parent_id: usize,
        call_fn_info: &inst::CallFuncMetaData,
        this_is_self: SelfPtrInfo,
    ) -> Result<String, err::ErrKind> {
        // 関数呼び出しのアセンブリが`build_fn_process`側で
        // 既に生成済みか
        let already_emitted = self.emitted_calls.contains(&parent_id);

        if !already_emitted && call_fn_info.parent != crate::ir::IS_ASSIGN_EXPR {
            // 呼び出しを生成する対象ではない
            return Ok(String::new());
        }
        if !already_emitted {
            let call_asm = self.emit_call_func(
                call_fn_info,
                matches!(this_is_self, Some(types::Size::Struct(..))),
            )?;
            self.asm_text.push_str(&call_asm);
        }
        // 戻り値のレジスタ(0番、`%eax`など)を返す。
        // 戻り値を受ける側の型(`this_is_self`)がある
        // 場合はそのサイズ、なければ64bit(`%rax`)
        let ret_size = this_is_self.map(Self::value_reg_size).unwrap_or(Size::DQ);
        if ret_size.is_float() {
            self.asm_fmt.get_fmt_float_reg(0, &ret_size)
        } else {
            self.asm_fmt.get_fmt_reg(0, &ret_size)
        }
    }

    /// 式などの計算結果を持つレジスタのオペランドを返す
    /// (`last_inst_idx`に結果のレジスタが記録されているノード)
    fn expr_result_operand(
        &mut self,
        parent_id: usize,
        node: &inst::Inst,
    ) -> Result<String, err::ErrKind> {
        if let Some(result) = self.last_inst_idx.iter().find(|i| i.0 == parent_id) {
            // 式の結果を持つレジスタは、その式自身の型のサイズで
            // 取得する(常に64bitにすると、`movl %rcx, %edx`の
            // ように32bitの命令へ64bitのレジスタが混ざる)
            let reg = result.1;
            let size = match node {
                inst::Inst::Expr(..) => Self::value_reg_size(&self.get_expr_ty(parent_id)),
                // キャストの結果は、キャスト先の型のサイズで参照する
                inst::Inst::Cast { after, .. } => Self::value_reg_size(after),
                _ => Size::DQ,
            };
            match reg {
                RegisterMode::Float(index) => self.asm_fmt.get_fmt_float_reg(index, &size),
                RegisterMode::Normal(index) => self.asm_fmt.get_fmt_reg(index, &size),
            }
        } else {
            panic!("{:?}", node);
        }
    }

    pub(super) fn format_expr_inst(
        &mut self,
        expr: &inst::ExprInst,
    ) -> Result<String, err::ErrKind> {
        // `/`と`%`は`%rax`/`%rdx`を使う専用の手順で生成する
        // (`div_ir.rs`)。テンプレートでは、割られる数を`%rax`ではなく
        // `%rdx`に置いてしまい、さらに`%rdx`にある引数を壊していた
        if matches!(expr.kind, inst::ExprKind::Div | inst::ExprKind::Surplus) {
            return self.format_div_expr_inst(expr);
        }

        let key = Self::expr_tmpl_key(&expr, &self.curr_inst)?;
        let mnemonic = Self::expr_mnemonic(&expr.kind)?;

        let resolved_size: Size = self.get_expr_ty(expr.ls);
        if matches!(expr.kind, inst::ExprKind::Inc | inst::ExprKind::Dec) {
            let is_memory_access = self.check_node_is_mem_val(expr.ls).is_some();
            let operand = self.extract_operand_text(expr.ls, None)?;
            let dst =
                self.get_reg_txt(Some(self.reg_idx), &Self::value_reg_size(&resolved_size))?;
            let mut output = self.mov_line(&dst, &operand, &resolved_size, is_memory_access);
            let mut update = self.asm_fmt.get_opcode_tmpl(key).replace("{dst}", &operand);
            update = if is_memory_access {
                self.asm_fmt
                    .fmt_memory_mnemonic_resize(mnemonic, &update, &resolved_size)
            } else {
                self.asm_fmt
                    .fmt_mnemonic_resize(mnemonic, &update, &resolved_size)
            };
            output.push_str(&update);
            return Ok(output);
        }
        // `<`/`>`は、どちらかのオペランドが符号なしなら符号なしの比較
        // (`jl`/`jg`ではなく`jb`/`ja`)にする。`==`/`!=`は符号に関係ない
        let is_unsigned_cmp = matches!(
            expr.kind,
            inst::ExprKind::LessThen
                | inst::ExprKind::GreaterThen
                | inst::ExprKind::LessThenEq
                | inst::ExprKind::GreaterThenEq
        ) && (resolved_size.is_unsigned()
            || self.get_expr_ty(expr.rs).is_unsigned());
        let wrap_size = resolved_size.wrap_dst_size();

        let dst_text = self.get_reg_txt(Some(self.reg_idx), &resolved_size)?;
        let src1_text = self.extract_operand_text(expr.ls, wrap_size.as_ref())?;

        let mut tmpl = if is_unsigned_cmp {
            self.asm_fmt.get_unsigned_cmp_tmpl(key)
        } else {
            self.asm_fmt.get_opcode_tmpl(key)
        };
        if expr.kind == inst::ExprKind::Mul {
            tmpl = tmpl.replace("add", "imul");
        }

        let src2_text = self.extract_operand_text(expr.rs, wrap_size.as_ref())?;
        let mut formated = tmpl
            .replace("{dst}", &dst_text)
            .replace("{src1}", &src1_text)
            .replace("{src2}", &src2_text)
            .to_string();

        let is_memory_access = self.check_node_is_mem_val(expr.ls).is_some()
            || self.check_node_is_mem_val(expr.rs).is_some();
        formated = self.fmt_one_expr_mnemo_resize(
            formated.to_string(),
            &resolved_size,
            mnemonic,
            is_memory_access,
        );
        if dst_text == src1_text
            && matches!(
                expr.kind,
                inst::ExprKind::Add
                    | inst::ExprKind::Sub
                    | inst::ExprKind::AndCalcul
                    | inst::ExprKind::XorCalcul
                    | inst::ExprKind::OrCalcul
                    | inst::ExprKind::LShift
                    | inst::ExprKind::RShift
                    | inst::ExprKind::Mul
                    | inst::ExprKind::LessThen
                    | inst::ExprKind::GreaterThen
                    | inst::ExprKind::LessThenEq
                    | inst::ExprKind::GreaterThenEq
                    | inst::ExprKind::Equal
                    | inst::ExprKind::NotEq
            )
        {
            let self_move = self.mov_line(&dst_text, &src1_text, &resolved_size, false);
            formated = formated.replacen(&self_move, "", 1);
        }

        // サイズがSelfでない場合
        if self.reserved_label_name.is_some() && formated.contains("{label}") {
            let name = self.reserved_label_name.take().unwrap();
            Ok(formated.replace("{label}", &name))
        } else {
            Ok(formated)
        }
    }
}
