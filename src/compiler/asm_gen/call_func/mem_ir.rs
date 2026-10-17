use super::*;
use crate::compiler::ir::types;

impl AsmEmitter {
    /// 変数の定義(`Inst::Mov`)のアセンブリを生成する
    ///
    /// 定義する変数の型と初期値によって、4つに分けて処理する
    /// 1. 構造体リテラルをポインタ変数へ束縛する(`mov_struct_literal_to_ptr`)
    /// 2. 構造体の変数(`mov_struct_var`)
    /// 3. 静的領域の値を、ポインタでない変数へ束縛する(`mov_static_var`)
    /// 4. それ以外はレジスタに置く変数(`mov_reg_var`)
    pub(super) fn mov_value_ir(
        &mut self,
        size: &types::Size,
        dst: usize,
        src: usize,
        name: Option<&str>,
        this_is_self: SelfPtrInfo,
    ) {
        if let Some(struct_ty) = self.struct_literal_ptr_target(size, src) {
            self.mov_struct_literal_to_ptr(size, struct_ty, dst, src, name);
        } else if let types::Size::Struct(_) = size {
            self.mov_struct_var(size, dst, src, name, this_is_self);
        } else if size.is_pointer().is_none() && self.static_label(src).is_some() {
            self.mov_static_var(size, dst, name);
        } else if let types::Size::Array { size: elem, len } = size {
            self.init_array_node(name, elem, *len, dst, src, this_is_self);
        } else {
            self.mov_reg_var(size, dst, src, name, this_is_self);
        }
    }

    /// 構造体リテラルをポインタ変数へ束縛する場合(`p: Name* = Name {..}`)、
    /// 指す先の構造体の型を返す。それ以外は`None`
    fn struct_literal_ptr_target<'a>(
        &self,
        size: &'a types::Size,
        src: usize,
    ) -> Option<&'a types::Size> {
        if let types::Size::Pointer { ty, .. } = size {
            if matches!(ty.as_ref(), types::Size::Struct(_))
                && matches!(self.inst_at(src), inst::Inst::Struct { is_self: false, .. })
            {
                return Some(ty.as_ref());
            }
        }
        None
    }

    /// 配列リテラル(`Inst::InitArr`)で初期化する配列変数を定義する
    ///
    /// 領域全体(`要素のサイズ * len`)を`stk_use_counter`から先に確保し、
    /// 要素`k`を`arr_base + (k + 1) * 要素のサイズ`へ書き込む。
    /// リテラルの要素数が`len`より少ない場合(`arr: [uint 3] = {0}`)、
    /// 残りの要素は0で埋める
    fn init_array_node(
        &mut self,
        name: Option<&str>,
        elem: &types::Size,
        len: usize,
        dst: usize,
        src: usize,
        this_is_self: SelfPtrInfo,
    ) {
        let inst::Inst::InitArr(elems) = self.inst_at(src) else {
            panic!(
                "配列の初期化式はInitArrである必要があります: {:?}",
                self.curr_inst[src]
            );
        };
        let elems: Vec<usize> = elems.clone();
        assert!(
            elems.len() <= len,
            "配列リテラルの要素数({})が配列の長さ({})を超えています",
            elems.len(),
            len
        );

        let elem_bytes = elem.to_bytes();
        let arr_base = self.stk_use_counter;
        self.stk_use_counter = arr_base + elem_bytes * len;

        let store_size = if elem.is_pointer().is_some() {
            Size::DQ
        } else {
            elem.clone()
        };

        let mut txt = String::new();
        for k in 0..len {
            let value = match elems.get(k) {
                Some(idx) => self.extract_operand_text(*idx, this_is_self),
                None => "$0".to_string(),
            };
            let elem_ref = self
                .asm_fmt
                .fmt_ref_operand(FRAME_BASE_REG, arr_base + (k + 1) * elem_bytes);
            txt.push_str(&self.mov_line(&elem_ref, &value, &store_size, true));
        }
        self.asm_text.push_str(txt.as_str());

        if let Some(var_name) = name {
            let array_ty = types::Size::Array {
                size: Box::new(elem.clone()),
                len,
            };
            self.insert_var_info(
                var_name,
                asm_emitter::VarIndexInfo::new(RegisterMode::Normal(self.reg_idx), &array_ty, dst)
                    .with_arr_base(arr_base),
            );
        }
        // 初期値が式の結果だった場合、そのレジスタはもう不要
        for idx in &elems {
            self.release_expr_temp(*idx);
        }
    }

    /// 変数が使うレジスタを決めて、使用中として記録する(`self.reg_idx`も更新する)
    ///
    /// `reg_mnger.rs`が使用回数をもとに割り当てたレジスタが空いていれば
    /// それを使い、そうでなければ使用中のレジスタ(引数など)と
    /// `%rax`/`%rdx`を避けて空きレジスタを確保する。
    /// このレジスタはスコープが終わると解放される
    fn bind_var_reg(&mut self, name: Option<&str>, size: &types::Size) -> RegisterMode {
        let reg = match name
            .and_then(|n| self.used_reg.allocated_reg(n))
            .filter(|r| !self.used_reg.is_used(r))
        {
            Some(reg) => reg,
            None => self.alloc_reg(size),
        };
        self.reg_idx = reg.index();
        match name {
            Some(var_name) => self.used_reg.bind_var(var_name, &reg),
            None => {
                self.used_reg.mark_used(&reg);
            }
        }
        reg
    }

    /// 構造体リテラルをポインタ変数へ束縛する
    ///
    /// 構造体の実体はスタックに置き、変数自体はその先頭アドレスを
    /// 保持するレジスタに置く
    fn mov_struct_literal_to_ptr(
        &mut self,
        size: &types::Size,
        struct_ty: &types::Size,
        dst: usize,
        src: usize,
        name: Option<&str>,
    ) {
        let offset = self
            .struct_mem(None, struct_ty, src, dst)
            .expect("構造体リテラルの初期化に失敗しました");
        let reg = self.bind_var_reg(name, size);

        let text = self.tmpl_line(
            "address",
            &self.reg64(&reg),
            &self.rbp_ref(offset),
            &types::Size::DQ,
            false,
        );
        self.asm_text.push_str(&text);
        if let Some(var_name) = name {
            self.insert_var_info(var_name, asm_emitter::VarIndexInfo::new(reg, size, dst));
        }
    }

    /// コンストラクタ呼び出しで初期化する構造体の、暗黙の`self`引数
    /// (`GetAddress(GetPtr)`)が指すスタック領域を、先に`stk_use_counter`から確保する
    ///
    /// IRの`stk`のままだと、先に確保済みの配列などと領域が重なってしまう
    fn reserve_ctor_self_stk(&mut self, src: usize, size: &types::Size) {
        if let inst::Inst::CallFunc(meta_data) = self.inst_at(src) {
            if let Some(&self_arg) = meta_data.params.get(0) {
                if let inst::Inst::GetAddress(mem_idx) = self.inst_at(self_arg) {
                    if matches!(self.curr_inst[mem_idx], inst::Inst::GetPtr { .. }) {
                        self.alloc_struct_stk(mem_idx, size.to_bytes());
                    }
                }
            }
        }
    }

    /// 構造体の変数(`s: Name = Name {..}` / `s: Name = Name::new()`)を定義する
    ///
    /// 実体はスタックに置き、変数にはレジスタではなく`%rbp`からの
    /// オフセットを登録する(`VarIndexInfo::new_stack`)
    fn mov_struct_var(
        &mut self,
        size: &types::Size,
        dst: usize,
        src: usize,
        name: Option<&str>,
        this_is_self: SelfPtrInfo,
    ) {
        self.reserve_ctor_self_stk(src, size);
        // 呼び出し自体(`lea`+`call`)は副作用として`self.asm_text`へ
        // 積まれる。戻り値のオペランド文字列自体は構造体には
        // 使えないので捨てる
        let _ = self.extract_operand_text(src, this_is_self);
        // 構造体リテラルの場合は`struct_mem`が初期化と登録まで行う
        if self.struct_mem(name, size, src, dst).is_some() {
            return;
        }
        // コンストラクタ呼び出しの場合は、`self`引数が指す領域を変数の実体として登録する
        let stk = self.ctor_self_stk_offset(src);
        if let Some(var_name) = name {
            self.insert_var_info(
                var_name,
                asm_emitter::VarIndexInfo::new_stack(stk, size, dst),
            );
        }
    }

    /// コンストラクタ呼び出し`src`の、暗黙の`self`引数が指すスタック領域の
    /// `%rbp`からのオフセットを返す
    fn ctor_self_stk_offset(&self, src: usize) -> usize {
        let inst::Inst::CallFunc(meta_data) = self.inst_at(src) else {
            panic!(
                "構造体を返す初期化式はコンストラクタ呼び出しである必要があります: {:?}",
                self.curr_inst[src]
            );
        };
        let self_arg_idx = *meta_data
            .params
            .get(0)
            .expect("構造体を返す関数は暗黙のselfポインタ引数を持つ必要があります");
        let inst::Inst::GetAddress(mem_idx) = self.inst_at(self_arg_idx) else {
            panic!("システムエラー: 暗黙のselfポインタ引数がGetAddressではありません");
        };
        let inst::Inst::GetPtr { stk, .. } = self.inst_at(mem_idx) else {
            panic!("システムエラー: 暗黙のselfポインタ引数の参照先がGetPtrではありません");
        };
        self.ptr_stk(mem_idx, stk)
    }

    /// 静的領域の値を、ポインタでない変数へ束縛する
    ///
    /// 変数の実体が静的領域そのものであり、レジスタへ値をロード
    /// する必要がないため、変数名だけを登録する
    fn mov_static_var(&mut self, size: &types::Size, dst: usize, name: Option<&str>) {
        self.insert_var_info(
            name.unwrap(),
            asm_emitter::VarIndexInfo::new(RegisterMode::Normal(self.reg_idx), size, dst),
        );
    }

    /// レジスタに置く変数を定義する
    fn mov_reg_var(
        &mut self,
        size: &types::Size,
        dst: usize,
        src: usize,
        name: Option<&str>,
        this_is_self: SelfPtrInfo,
    ) {
        let reg = self.bind_var_reg(name, size);
        let formated = self.init_reg_line(size, reg, src, this_is_self);
        self.asm_text.push_str(&formated);
        // 初期値が式の結果だった場合、そのレジスタはもう不要
        self.release_expr_temp(src);

        if let Some(var_name) = name {
            self.insert_var_info(var_name, asm_emitter::VarIndexInfo::new(reg, size, dst));
            let current_reg = reg;

            if self
                .expr_vars
                .iter()
                .find(|v| v.as_str() == var_name)
                .is_some()
            {
                self.update_value_reg(var_name, &current_reg);
            }
        }
    }

    /// レジスタ`reg`へ初期値`src`を書き込む命令を作る
    fn init_reg_line(
        &mut self,
        size: &types::Size,
        reg: RegisterMode,
        src: usize,
        this_is_self: SelfPtrInfo,
    ) -> String {
        // ポインタ型の変数へ数値リテラル(`ptr: int* = 0`の
        // ような`0`=NULL初期化など)を代入する場合は、
        // アドレスを求める`lea`(="address")命令ではなく、
        // ポインタのサイズ(64bit)に合わせた`movq`で
        // そのまま即値をレジスタへ書き込む
        let is_literal_num = matches!(self.curr_inst[src], inst::Inst::Num { .. });

        if size.is_pointer().is_some() && is_literal_num {
            let dst_reg = self.reg64(&reg);
            let value_operand = self.extract_operand_text(src, this_is_self);
            self.mov_line(&dst_reg, &value_operand, &Size::DQ, false)
        } else {
            // メモリのポインタか、値かで、ニーモニックが変わる
            let mnemonic = if size.is_pointer().is_some() {
                // ポインタの場合
                "address"
            } else if size.is_float() {
                match size {
                    types::Size::F32 => "float_mov",
                    types::Size::F64 => "f64_mov",
                    _ => unreachable!(),
                }
            } else {
                "mov"
            };
            match reg {
                RegisterMode::Float(index) if size.is_float() => {
                    let opcode = if matches!(size, types::Size::F32) {
                        "float_mov"
                    } else {
                        "f64_mov"
                    };
                    let dst = self.asm_fmt.get_fmt_float_reg(index, size);
                    let src = self.extract_operand_text(src, this_is_self);
                    self.asm_fmt
                        .get_opcode_tmpl(opcode)
                        .replace("{src1}", &src)
                        .replace("{dst}", &dst)
                }
                RegisterMode::Normal(index) => {
                    if size.is_float() {
                        panic!("floating-point variable allocated to a normal register");
                    }
                    self.format_line(mnemonic, Some(index), src, None, this_is_self)
                }
                RegisterMode::Float(_) => {
                    panic!("non-floating-point variable allocated to a floating-point register")
                }
            }
        }
    }

    /// メモリに置く値(`Inst::MemoryValue`)のアセンブリを生成する
    ///
    /// - 静的領域(`MemoryKind::Static`): `is_static_var`
    /// - それ以外(配列): `stack_arr_ir`
    pub(super) fn mem_val_ir(&mut self, mem_value: &inst::MemoryInst) {
        match mem_value {
            inst::MemoryInst::Memory {
                name,
                size,
                src,
                kind,
                dst,
            } => {
                if kind == &inst::MemoryKind::Static {
                    self.is_static_var(src, *dst, name, size.wrap_dst_size().as_ref());
                } else {
                    self.stack_arr_ir(name, size, src, *dst);
                }
            }
            _ => panic!(),
        }
    }

    /// スタックに置く配列の、各要素の書き込みを生成して変数として登録する
    fn stack_arr_ir(&mut self, name: &str, size: &types::Size, src: &[usize], dst: usize) {
        let dst_size = size.wrap_dst_size();
        let base = match &self.curr_inst[dst] {
            inst::Inst::Pointer(..) | inst::Inst::Param(..) | inst::Inst::GetPtr { .. } => {
                self.extract_operand_text(dst, dst_size.as_ref())
            }
            _ => FRAME_BASE_REG.to_string(),
        };

        let mut txt = String::new();
        // 要素0が最も%rbpに近い位置(`-4(%rbp)`)に来て、要素が
        // 増えるほどアドレスが小さくなる(`-4`, `-8`, `-12`, ...)。
        // 領域全体(`要素のサイズ * 要素数`)を先に確保し、
        // 要素`k`のオフセットは `arr_base + (k + 1) * 要素のサイズ`
        let elem_bytes = size.to_bytes();
        let arr_base = self.stk_use_counter;
        self.stk_use_counter = arr_base + elem_bytes * src.len();
        for (k, idx) in src.iter().enumerate() {
            let value = self.extract_operand_text(*idx, dst_size.as_ref());
            let elem_ref = self
                .asm_fmt
                .fmt_ref_operand(&base, arr_base + (k + 1) * elem_bytes);
            let store_size = if size.is_pointer().is_some() {
                Size::DQ
            } else {
                size.clone()
            };
            txt.push_str(&self.mov_line(&elem_ref, &value, &store_size, true));
        }
        self.insert_var_info(
            name,
            asm_emitter::VarIndexInfo::new(RegisterMode::Normal(self.reg_idx), size, dst)
                .with_arr_base(arr_base),
        );
        self.asm_text.push_str(txt.as_str());
    }

    fn is_static_var(&mut self, src: &[usize], dst: usize, name: &str, this_is_self: SelfPtrInfo) {
        println!("src/gen/call_func/MemoryValue");
        let val = self.extract_operand_text(*src.last().unwrap(), this_is_self);
        let label_name = format!("M{}", self.data_idx.to_string());
        let fmt_data =
            self.asm_fmt
                .get_static_num_fmt(&val, &label_name, this_is_self.as_ref().unwrap());
        self.data_sec_text.push_str(&fmt_data);
        self.data_map.push((dst, label_name));
        self.data_idx += 1;
        // 子のノードがstaticりょいきの値なので、変数名だけ登録する
        self.insert_var_info(
            &name,
            asm_emitter::VarIndexInfo::new(
                RegisterMode::Normal(self.reg_idx),
                this_is_self.as_ref().unwrap(),
                dst,
            ),
        );
    }

    fn struct_mem(
        &mut self,
        name: Option<&str>,
        size: &types::Size,
        src: usize,
        dst: usize,
    ) -> Option<usize> {
        if let inst::Inst::Struct { mem, is_self, .. } = self.inst_at(src) {
            // `emit_struct_ini_asm`はメンバーを書き込みながら
            // `self.stk_use_counter`を進めていくため、呼び出し後
            // では構造体自身の先頭オフセット(`%rbp`から見た位置)が
            // 分からなくなってしまう。呼び出し前の値を控えておき、
            // それをこの変数の実体の位置として登録する
            // 構造体のポインタは先頭のメンバーを指すので、`alloc_struct_stk`と
            // 同様に、先頭のメンバーがはみ出す分(8byte)を余分に確保する
            let struct_stk_offset = if is_self {
                self.stk_use_counter
            } else {
                let base = self.stk_use_counter.div_ceil(8) * 8 + 8;
                self.stk_use_counter = base;
                base
            };
            let ini_asm = self.emit_struct_init_asm(mem, is_self);
            if !is_self {
                self.stk_use_counter = self
                    .stk_use_counter
                    .max(struct_stk_offset + size.to_bytes());
            }
            self.asm_text.push_str(ini_asm.as_str());
            if let Some(var_name) = name {
                self.insert_var_info(
                    var_name,
                    asm_emitter::VarIndexInfo::new_stack(struct_stk_offset, &size, dst),
                );
            }
            return Some(struct_stk_offset);
        }
        None
    }
}
