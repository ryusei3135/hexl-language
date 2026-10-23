mod assign_var;
mod mem_ir;

use super::*;
use crate::compiler::asm_gen::{self, emit_fn_name::*};
use crate::compiler::ir::{self, types};
use crate::compiler::{ast, err};

/// 関数の入口で`callee_saved`のレジスタを退避する位置の目印
/// (本体の生成が終わってから、実際に使ったレジスタの`push`に置き換える)
const CALLEE_SAVE_MARK: &str = "{callee_save}";
/// `leave`/`ret`の直前で、退避したレジスタを戻す位置の目印
const CALLEE_RESTORE_MARK: &str = "{callee_restore}";

/// 1つの関数のアセンブリを生成している間、各ノードの生成で共通して
/// 参照する情報(`build_fn_process`で作り、`gen_node_asm`へ渡す)
struct FnGenCtx<'a> {
    /// 生成中の関数の名前(`_start`かどうかの判定に使う)
    fn_name: &'a str,
    /// 関数の戻り値の型(戻り値が無い/構造体を返す関数は`None`)
    fn_ret_ty: SelfPtrInfo<'a>,
    /// 先頭の引数が`self`か
    this_is_self: bool,
    /// 戻り値として返される構造体の`Inst::Struct`のid
    returned_struct_idx: Option<usize>,
    /// 出力するアセンブリ言語のフォーマットの名前
    asm_fmt_name: &'a Option<&'a str>,
}

impl AsmEmitter {
    /// 関数のラベル名を作る。ジェネリクス関数は型引数を名前に含める
    pub(super) fn fn_label_of<T: std::fmt::Debug>(name: &str, temp_ty: &[T]) -> String {
        if temp_ty.is_empty() {
            emit_fn_name_id(name)
        } else {
            let generic_args = temp_ty
                .iter()
                .map(|ty| format!("{:?}", ty))
                .collect::<Vec<_>>();
            emit_generic_fn_name_id(name, &generic_args)
        }
    }

    /// 関数を呼び出す情報がある物を受け取りアセンブリ言語を生成する
    ///
    /// `gen/asm_emitter.rs`の`extract_operand_text`から、関数呼び出しの
    /// 結果を値として使う(戻り値を任意のレジスタへ代入する)際にも
    /// 使われるため`pub(super)`にしている
    ///
    /// 流れ: callerレジスタの退避 → 引数の設定 → `call` → 退避したレジスタの復元
    pub(super) fn emit_call_func(
        &mut self,
        meta_data: &inst::CallFuncMetaData,
        expand_struct_return: bool,
    ) -> Result<String, err::ErrKind> {
        // 呼ぶ関数がまだ生成されていなければ、ここで`build_fn_process`で生成する
        // (呼んだ側の生成中の状態は退避・復元するので影響しない)
        let fn_label = Self::fn_label_of(&meta_data.name, &meta_data.temp_ty);
        if let Some(mut callee) = self.pending_funcs.remove(&fn_label) {
            self.emit_callee_fn(&mut callee);
        }

        // 生成するアセンブリコード
        let mut call_func = String::new();

        // 関数を呼ぶためレジスタを退避
        let protected_regs: Vec<RegisterMode> = self
            .used_reg
            .used_regs()
            .into_iter()
            .filter(|reg| *reg != RegisterMode::Normal(0))
            .collect();
        // 変数のレジスタは領域へ`mov`で退避し、その変数が参照されるまで復元しない
        // (`lazy_spill.rs`)。退避中のレジスタは中身が壊れているので保存しない。
        // それ以外(式の一時的な結果など)は、従来どおり`push`/`pop`
        let mut lazy_regs: Vec<RegisterMode> = Vec::with_capacity(protected_regs.len());
        let mut eager_regs: Vec<RegisterMode> = Vec::with_capacity(protected_regs.len());
        for reg in protected_regs {
            if self.spilled_regs.contains_key(&reg) || self.is_callee_saved(reg) {
                continue;
            }
            if self.can_spill_lazily(reg) {
                lazy_regs.push(reg);
            } else {
                eager_regs.push(reg);
            }
        }
        call_func.push_str(&self.spill_store_regs(&lazy_regs)?);
        call_func.push_str(&self.push_caller_saved_regs(&eager_regs)?);

        // 関数の引数に使ったレジスタを解放するためにスコープを作ったことにする
        self.used_reg.start_scope();
        let arg_lines = self.build_arg_lines(meta_data, expand_struct_return)?;
        call_func.push_str(&self.order_arg_lines(arg_lines)?);

        call_func.push_str(&self.asm_fmt.get_call_func_fmt(&fn_label));
        // 引数で使ったレジスタを解放
        self.used_reg.end_scope();

        // call_func.push_str(&self.pop_caller_saved_regs(&eager_regs));
        call_func.push_str(&self.after_call_spills(&lazy_regs)?);
        Ok(call_func)
    }

    /// 呼ばれた関数1つぶんを`build_fn_process`で生成し、`fn_asm_out`へ溜める
    ///
    /// 呼んだ側の関数の生成の途中で呼ばれることがあるため、関数ごとの状態
    /// (変数・レジスタ・スタック使用量・IRなど)と`asm_text`を退避し、
    /// 生成が終わったら元に戻す。呼ばれた関数の中で、さらに別の関数を
    /// 呼ぶ場合も同じ処理で再帰的に生成される
    pub(super) fn emit_callee_fn(&mut self, callee: &mut (String, def_tree::FnDefInfo)) {
        let asm_text = mem::take(&mut self.asm_text);
        let var_hash_map = mem::take(&mut self.var_hash_map);
        let struct_stk_map = mem::take(&mut self.struct_stk_map);
        let expr_vars = mem::take(&mut self.expr_vars);
        let reserved_label_name = self.reserved_label_name.take();
        let curr_inst = mem::take(&mut self.curr_inst);
        let data_map = mem::take(&mut self.data_map);
        let last_inst_idx = mem::take(&mut self.last_inst_idx);
        let scope_outer_vars = mem::take(&mut self.scope_outer_vars);
        let emitted_calls = mem::take(&mut self.emitted_calls);
        let unused_var_nodes = mem::take(&mut self.unused_var_nodes);
        let arr_index_temp = self.arr_index_temp.take();
        let spilled_regs = mem::take(&mut self.spilled_regs);
        let spill_slots = mem::take(&mut self.spill_slots);
        let node_vars = mem::take(&mut self.node_vars);
        let node_vars_all = self.node_vars_all;
        let dead_vars = mem::take(&mut self.dead_vars);
        let used_reg = self.used_reg.clone();
        let reg_idx = self.reg_idx;
        let stk_use_counter = self.stk_use_counter;

        let asm_fmt_name = self.asm_fmt_name.clone();
        self.build_fn_process(callee, asm_fmt_name.as_deref());
        let callee_asm = mem::replace(&mut self.asm_text, asm_text);
        self.fn_asm_out.push_str(&callee_asm);

        self.var_hash_map = var_hash_map;
        self.struct_stk_map = struct_stk_map;
        self.expr_vars = expr_vars;
        self.reserved_label_name = reserved_label_name;
        self.curr_inst = curr_inst;
        self.data_map = data_map;
        self.last_inst_idx = last_inst_idx;
        self.scope_outer_vars = scope_outer_vars;
        self.emitted_calls = emitted_calls;
        self.unused_var_nodes = unused_var_nodes;
        self.arr_index_temp = arr_index_temp;
        self.spilled_regs = spilled_regs;
        self.spill_slots = spill_slots;
        self.node_vars = node_vars;
        self.node_vars_all = node_vars_all;
        self.dead_vars = dead_vars;
        self.used_reg = used_reg;
        self.reg_idx = reg_idx;
        self.stk_use_counter = stk_use_counter;
    }

    /// `regs`のうちcallerが保存するレジスタだけを`push`するアセンブリを返す
    /// (`callee_saved`のレジスタは呼ばれた側が保存するので退避しない)
    fn is_callee_saved(&self, reg: RegisterMode) -> bool {
        match reg {
            RegisterMode::Normal(index) => self.callee_saved.contains(&index),
            RegisterMode::Float(index) => self.float_callee_saved.contains(&index),
        }
    }

    fn push_caller_saved_regs(&self, regs: &[RegisterMode]) -> Result<Vec<String>, err::ErrKind> {
        let emit_caller_reg = |reg: &asm_gen::RegisterMode| -> Result<String, err::ErrKind>{
            Ok(match reg {
                RegisterMode::Normal(index) => {
                    let reg_name = self.asm_fmt.get_fmt_reg(*index, &Size::DQ)?;
                    self.asm_fmt.get_push(&reg_name)
                }
                RegisterMode::Float(_) => {
                    let size = self.float_reg_size(*reg);
                    let opcode = Self::float_move_opcode(&size);
                    let operand = self.reg_mode_operand(*reg, &size)?;
                    format!(
                        "sub $16, %rsp\n{}",
                        self.asm_fmt
                            .get_opcode_tmpl(opcode)
                            .replace("{src1}", &operand)
                            .replace("{dst}", "(%rsp)")
                    )
                }
            })
        };
        let result = regs
            .iter()
            .filter(|reg| !self.is_callee_saved(**reg))
            .map(emit_caller_reg)
            .collect();
        Ok(result)
    }

    // /// `push_caller_saved_regs`で退避したレジスタを、退避した時とは逆順に復元する
    // fn pop_caller_saved_regs(&self, regs: &[RegisterMode]) -> String {
    //     regs.iter()
    //         .rev()
    //         .filter(|reg| !self.is_callee_saved(**reg))
    //         .map(|reg| {
    //             match reg {
    //                 RegisterMode::Normal(index) => {
    //                     let reg_name = self.asm_fmt.get_fmt_reg(*index, &Size::DQ);
    //                     self.asm_fmt.get_pop(&reg_name)
    //                 }
    //                 RegisterMode::Float(_) => {
    //                     let size = self.float_reg_size(*reg);
    //                     let opcode = Self::float_move_opcode(&size);
    //                     let operand = self.reg_mode_operand(*reg, &size);
    //                     let restore = self
    //                         .asm_fmt
    //                         .get_opcode_tmpl(opcode)
    //                         .replace("{src1}", "(%rsp)")
    //                         .replace("{dst}", &operand);
    //                     format!("{restore}add $16, %rsp\n")
    //                 }
    //             }
    //         })
    //         .collect::<String>()
    // }

    /// 引数`param`を引数レジスタへ渡す命令の種類を決める
    ///
    /// - `"address"`: 値ではなくアドレスを渡す(`lea`)
    /// - `"mov"`: 値をそのまま渡す
    fn arg_opcode(&self, param: usize, index: usize, expand_struct_return: bool) -> &'static str {
        if expand_struct_return && index == 0 {
            "address"
        } else if self.curr_inst[param].is_pointer() {
            "address"
        } else if self.curr_inst[param].is_float(&self.curr_inst) {
            match self.curr_inst[param].get_param_ty() {
                Some(types::Size::F32) => "float_mov",
                Some(types::Size::F64) => "f64_mov",
                _ => panic!("floating-point argument has non-floating-point type"),
            }
        } else if matches!(self.curr_inst[param], inst::Inst::Str { .. }) {
            // 文字列の実体は静的領域(`M0`など)にあり、引数には
            // その「アドレス」を渡す必要がある。`mov`だとラベルの指す
            // 中身を読み込んでしまうので、`leaq M0(%rip), %rdi`のように
            // `lea`(=`address`)でアドレスを求める
            "address"
        } else {
            "mov"
        }
    }

    /// 各引数を引数レジスタへ設定する命令を作る
    ///
    /// 戻り値は各引数の`(引数レジスタの番号, 値の読み出し元のテキスト, 生成したasm)`
    /// 並び順は引数の順で、実際の出力順は`order_arg_lines`で決める
    fn build_arg_lines(
        &mut self,
        meta_data: &inst::CallFuncMetaData,
        expand_struct_return: bool,
    ) -> Result<Vec<(RegisterMode, String, String)>, err::ErrKind> {
        let mut arg_lines: Vec<(RegisterMode, String, String)> = Vec::new();

        for (index, param) in meta_data.params.iter().enumerate() {
            // 引数の実際の型のサイズ
            // (レジスタの取得だけでなく、後段のニーモニックの
            //  サイズ決定にも同じサイズを使う必要がある)
            let param_ty = self.curr_inst[*param].get_param_ty();
            // 引数のレジスタを取得
            let param_reg = self.asm_fmt.get_fmt_param::<String>(index, &param_ty)?;
            // 引数のレジスタと値のidを挿入
            let src1_idx = if let Some(struct_idx) = self.resolve_struct_idx(*param) {
                struct_idx
            } else {
                *param
            };
            let param_reg_mode = self.arg_register_mode(index, &param_ty);
            self.used_reg.mark_used(&param_reg_mode);

            let opcode = self.arg_opcode(*param, index, expand_struct_return);

            let resize_size = if opcode == "address" {
                Size::DQ
            } else {
                param_ty.clone()
            };

            let src1_text =
                self.extract_operand_text(src1_idx, resize_size.wrap_dst_size().as_ref())?;
            let src1_text = self.asm_fmt.resize_reg_operand(&src1_text, &resize_size)?;

            let is_memory = self.check_node_is_mem_val(*param).is_some();
            let asm = self.tmpl_line(opcode, &param_reg, &src1_text, &resize_size, is_memory);
            let param_reg_num = self.asm_fmt.get_fmt_param::<usize>(index, &param_ty)?;
            let param_reg_num = if param_ty.is_float() {
                RegisterMode::Float(param_reg_num)
            } else {
                RegisterMode::Normal(param_reg_num)
            };
            arg_lines.push((param_reg_num, src1_text, asm));
        }
        Ok(arg_lines)
    }

    /// 引数レジスタへ値を設定する命令を、レジスタを壊さない順序に並べて返す
    ///
    /// 先に`%rdi`へ値を書いてしまうと、後ろの引数が元の`%rdi`の値を
    /// 読む場合(`f(y, x)`で、`x`が`%rdi`にある場合など)に、
    /// 壊れた値を読んでしまう。「他の未出力の引数から読まれていない
    /// 引数レジスタ」を持つ引数から順に出力する。
    /// (引数同士が循環している場合だけは、元の順のまま出力する)
    fn order_arg_lines(
        &self,
        arg_lines: Vec<(RegisterMode, String, String)>,
    ) -> Result<String, err::ErrKind> {
        let mut asm = String::new();
        let mut pending: Vec<usize> = (0..arg_lines.len()).collect();

        while !pending.is_empty() {
            let pos = pending
                .iter()
                .position(|i| {
                    (0..arg_lines.len()).all(|j| {
                        j == *i || !self.text_reads_reg(&arg_lines[j].1, arg_lines[*i].0)?
                    })
                })
                .unwrap_or(0);
            let i = pending.remove(pos);
            asm.push_str(&arg_lines[i].2);
        }
        Ok(asm)
    }

    /// 関数のメタ情報のbodyのデータを元に
    /// アセンブリ言語をフォーマットに沿って生成する関数
    ///
    /// ## 引数
    /// - func_meta_data
    /// - asm_fmt_name
    ///     出力するアセンブリ言語のフォーマットの名前
    ///
    /// ## 流れ
    /// 1. ラベルを出力し、関数ごとの状態を初期化する(`begin_fn_state`)
    /// 2. 仮のスタックフレームを出力し、位置を控える
    /// 3. IRのノードを順に変換する(`gen_node_asm`)
    /// 4. `ret`が無ければ付ける(`gen_implicit_ret`)
    /// 5. `callee_saved`の退避/復元を確定する(`fix_callee_saved`)
    /// 6. スタックフレームを実際の使用量で作り直す
    pub(super) fn build_fn_process(
        &mut self,
        fn_meta_data: &mut (String, def_tree::FnDefInfo),
        asm_fmt_name: Option<&str>,
    ) {
        let this_is_self = fn_meta_data.1.first_param_is_self();
        let fn_ret_ty = fn_meta_data.1.get_ret_ty();
        // 新しく関数の作成、
        let fn_label = Self::fn_label_of(&fn_meta_data.1.name, &fn_meta_data.1.temp_ty);
        self.asm_text.push_str(&format!("{}:\n", &fn_label));
        self.begin_fn_state(&fn_meta_data.1);

        // 関数の先頭のスタックフレームの生成位置を控えておく。
        // 本体を生成しながらスタックの使用量を数え、最後にこの位置の
        // スタックフレームを、数えた使用量(アライメント済み)で作り直す
        // (使用量は本体を生成してみないと確定しないため)
        let prologue_start = self.asm_text.len();
        let ir_stk_size = fn_meta_data.1.stk_size;
        let prologue = self.gen_fn_prologue(&fn_meta_data.0, ir_stk_size);
        self.asm_text.push_str(&prologue);
        let prologue_end = self.asm_text.len();
        // 使った`callee_saved`のレジスタの退避(どのレジスタを使うかは
        // 本体を生成し終わるまで分からないので、目印を置いて後で置き換える)
        if fn_meta_data.0 != "_start" {
            self.asm_text.push_str(CALLEE_SAVE_MARK);
        }

        self.curr_inst = mem::take(&mut fn_meta_data.1.body);
        let returned_struct_idx = self.find_returned_struct_idx(fn_ret_ty.is_none());
        let ctx = FnGenCtx {
            fn_name: &fn_meta_data.0,
            fn_ret_ty: fn_ret_ty.as_ref(),
            this_is_self,
            returned_struct_idx,
            asm_fmt_name: &asm_fmt_name,
        };
        // ノードの生成でエラーが返ってきたら、`err_pool`へ入れて次のノードへ進む
        // (1つの関数の中の、独立した複数のエラーをまとめて報告するため)
        for (node_idx, node) in self.curr_inst.clone().iter().enumerate() {
            if let Err(err_node) = self.gen_node_asm(node_idx, node, &ctx) {
                self.err_pool.borrow_mut().push_err_node(err_node);
            }
        }
        self.gen_implicit_ret(ctx.fn_name);
        self.fix_callee_saved(prologue_end);

        // === スタックフレームで確保するサイズを確定させる ===
        // IRが見積もったサイズ(`stk_size`)と、本体の生成で実際に数えた
        // スタックの使用量(`stk_use_counter`)の大きい方を使う。
        // `gen_stack_frame`が8/16byte境界へ切り上げる
        let used_stk_size = self.stk_use_counter.max(ir_stk_size);
        let prologue = self.gen_fn_prologue(ctx.fn_name, used_stk_size);
        self.asm_text
            .replace_range(prologue_start..prologue_end, &prologue);
    }

    /// 関数ごとの状態を初期化し、変数へレジスタを割り当てる
    fn begin_fn_state(&mut self, fn_def: &def_tree::FnDefInfo) {
        // 前の関数の変数・レジスタの状態を持ち越さない
        self.reset_fn_state();
        // 引数のレジスタと名前を登録
        self.setting_args_reg_alloc(&fn_def.args);
        // 変数の使用頻度に合わせてレジスタを割り当てる(`reg_mnger.rs`)
        // この関数自身の引数のレジスタには、まだ引数の値が入っているので使わない
        let param_regs: Vec<RegisterMode> = fn_def
            .args
            .iter()
            .enumerate()
            .map(|(idx, arg)| {
                self.arg_register_mode(idx, &arg.ty.clone().try_into().unwrap_or(Size::DQ))
            })
            .collect();
        // 構造体の変数(`s: Name = Name { .. }`)はスタックに置くので、
        // レジスタの割り当てから外し、次の変数に割り当てる
        let stack_vars = reg_mnger::UsedRegManager::stack_var_names(&fn_def.body);
        self.used_reg.allocation_register(
            &fn_def.var_counter,
            &param_regs,
            &stack_vars,
            &fn_def.body,
        );
        // 使用回数が0回の変数は、アセンブリを生成しない
        self.unused_var_nodes =
            reg_mnger::UsedRegManager::unused_var_nodes(&fn_def.var_counter, &fn_def.body);
        // 生成しない変数に割り当てたレジスタは取り消す
        // (同じ名前の定義が1つでも生成される場合は取り消さない)
        for (idx, node) in fn_def.body.iter().enumerate() {
            if let inst::Inst::Mov {
                name: Some(name), ..
            } = node
            {
                let all_removed = fn_def
                    .body
                    .iter()
                    .enumerate()
                    .all(|(i, other)| match other {
                        inst::Inst::Mov {
                            name: Some(other_name),
                            ..
                        } if other_name == name => self.unused_var_nodes.contains(&i),
                        _ => true,
                    });
                if self.unused_var_nodes.contains(&idx) && all_removed {
                    self.used_reg.release_allocation(name);
                }
            }
        }
        // 関数ごとにスタックの使用量をリセットする
        // (前の関数の`stk_use_counter`を持ち越すと、この関数の
        //  ローカル変数のオフセットが正しく計算できない)
        self.stk_use_counter = 0;
        // 生成済みの関数呼び出しの記録も、関数ごとにリセットする
        self.emitted_calls.clear();
    }

    fn arg_register_mode(&self, index: usize, arg_ty: &Size) -> RegisterMode {
        let reg = self.asm_fmt.get_fmt_param::<usize>(index, &arg_ty).unwrap();
        if arg_ty.is_float() {
            RegisterMode::Float(reg)
        } else {
            RegisterMode::Normal(reg)
        }
    }

    /// 関数の引数を名前とセットでレジスタを登録
    /// 引数が構造体などなら必ず64bitのサイズになる
    fn setting_args_reg_alloc(&mut self, args_def: &[ast::ArgsNode]) {
        args_def.iter().enumerate().for_each(|(idx, arg)| {
            let reg = self.arg_register_mode(idx, &arg.ty.clone().try_into().unwrap_or(Size::DQ));
            self.used_reg.bind_var(&arg.name, &reg);
        });
    }

    /// 戻り値として返される構造体の`Inst::Struct`のidを探す
    ///
    /// `fn_returns_nothing`は、関数の戻り値の型が無い(構造体は
    /// 暗黙の`self`ポインタ経由で返す)場合に`true`
    fn find_returned_struct_idx(&self, fn_returns_nothing: bool) -> Option<usize> {
        if !fn_returns_nothing {
            return None;
        }
        self.curr_inst.iter().find_map(|node| match node {
            inst::Inst::Ret(idx) => self.resolve_struct_idx(*idx),
            _ => None,
        })
    }

    /// IRのノード1つぶんのアセンブリを生成する
    ///
    /// 生成できないノードは`Err`を返す(`build_fn_process`が`err_pool`へ入れる)
    fn gen_node_asm(
        &mut self,
        node_idx: usize,
        node: &inst::Inst,
        ctx: &FnGenCtx,
    ) -> Result<(), err::ErrKind> {
        // 退避した変数のうち、このノードが参照するものを復元する(`lazy_spill.rs`)
        self.prepare_node_spills(node);
        match node {
            inst::Inst::ExpectJmp(name) => self.expect_jmp(name),
            // 生成しない変数の初期値だけを構成する式・定数は生成しない
            // (`unused_var_nodes`)
            inst::Inst::Expr(_)
            | inst::Inst::Cast { .. }
            | inst::Inst::Float { .. }
            | inst::Inst::Str { .. }
                if self.unused_var_nodes.contains(&node_idx) => {}
            // `asm_emitter/cast.rs`
            inst::Inst::Cast { src, after } => self.gen_cast_asm(node_idx, *src, after)?,
            inst::Inst::Str { dst, value } => self.gen_str_asm(*dst, value),
            inst::Inst::Expr(expr) => self.gen_expr_asm(expr)?,
            inst::Inst::Num { .. } => {}
            inst::Inst::Float { dst, value, size } => self.gen_float_asm(*dst, value, size),
            inst::Inst::InitArr(_) => {}
            inst::Inst::Block(name) => {
                self.asm_text.push_str(&format!("{}:\n", name));
            }
            inst::Inst::StartScope => self.start_scope_asm(),
            inst::Inst::EndScope => self.end_scope_asm(),
            inst::Inst::Comple { name, lines } => {
                self.gen_complie_asm(name, lines, ctx.asm_fmt_name.as_deref())
            }
            inst::Inst::Jmp(name) => {
                // 直前が無条件ジャンプ(`break`の`jmp L1`など)なら、
                // 次のラベルまで到達できないので`jmp`を生成しない
                let after_jmp = self
                    .asm_text
                    .lines()
                    .rev()
                    .map(str::trim)
                    .find(|l| !l.is_empty())
                    .is_some_and(|l| l.starts_with("jmp "));
                if !after_jmp {
                    self.asm_text.push_str(&format!("jmp {}\n", name));
                }
            }
            inst::Inst::AssignVar { name, dst, value } => {
                self.gen_assign_var_asm(name, *dst, *value, ctx.this_is_self)?
            }
            inst::Inst::Ret(idx) => {
                self.gen_ret_asm(ctx.fn_ret_ty, *idx, ctx.fn_name)?;
                // `ret`の後は次のラベルまで到達しない。退避は捨てる
                self.spilled_regs.clear();
            }
            inst::Inst::Mov {
                name,
                size,
                dst,
                src,
            } => {
                // 使用回数が0回の変数(`unused_var_nodes`)は生成しない
                if !self.unused_var_nodes.contains(&node_idx) {
                    self.gen_mov_node_asm(
                        name.as_deref(),
                        size,
                        *dst,
                        *src,
                        ctx.returned_struct_idx,
                    )
                }
            }
            // メモリに配置されている値の生成
            inst::Inst::MemoryValue(mem_value) => self.mem_val_ir(mem_value),
            inst::Inst::Param(_) => self.gen_param_asm(node),
            inst::Inst::CallFunc(meta_data) => self.gen_call_node_asm(node_idx, meta_data)?,
            _ => {}
        }
        Ok(())
    }

    /// `Inst::Mov`(変数の定義)のアセンブリを生成する
    fn gen_mov_node_asm(
        &mut self,
        name: Option<&str>,
        size: &types::Size,
        dst: usize,
        src: usize,
        returned_struct_idx: Option<usize>,
    ) {
        // 戻り値として返される構造体の初期化かどうか。
        //
        // `returned_struct_idx`が`None`(=この関数は構造体を返さない)
        // の場合に、`resolve_struct_idx(dst)`も`None`(=`dst`が
        // `a: A = A::new()`のようなコンストラクタ呼び出しで、
        // `Inst::Struct`を直接指していない)だと、`None == None`で
        // 「返される構造体」と誤判定されていた。その結果
        // `mov_value_ir`が呼ばれず、変数`a`が`var_hash_map`へ
        // 登録されないまま、以降の参照で「存在しない変数」に
        // なってしまっていた。
        // そのため、構造体が実際に返される場合(`Some`)のみ比較する
        let is_returned_struct = matches!(size, types::Size::Struct(..))
            && returned_struct_idx.is_some()
            && returned_struct_idx == self.resolve_struct_idx(dst);
        if !is_returned_struct {
            self.mov_value_ir(size, dst, src, name, Some(size));
            // 定義した変数のレジスタは新しい値になる
            if let Some(reg) = name.and_then(|n| self.var_reg(n)) {
                self.spilled_regs.remove(&reg);
            }
        }
    }

    /// `Inst::Param`(関数の引数)を変数として登録する
    fn gen_param_asm(&mut self, node: &inst::Inst) {
        let inst::Inst::Param(param) = node else {
            return;
        };
        let ty = node.get_param_ty().unwrap();
        // 引数に使うレジスタを取得する
        let reg_num = self.arg_register_mode(param.num, &ty);
        self.insert_var_info(
            &param.name,
            asm_emitter::VarIndexInfo::new(reg_num, &ty.try_into().unwrap(), param.dst),
        );
    }

    /// 関数を呼ぶノード(`Inst::CallFunc`)のアセンブリを生成する
    fn gen_call_node_asm(
        &mut self,
        node_idx: usize,
        meta_data: &inst::CallFuncMetaData,
    ) -> Result<(), err::ErrKind> {
        // 関数を呼ぶノードが変数に戻り値を代入しないばあいのみ生成
        if meta_data.parent == ir::IS_NOT_ASSIGN_EXPR {
            let asm_text = self.emit_call_func(meta_data, false)?;
            self.asm_text.push_str(asm_text.as_str());
            // 値として参照された場合(`a: int = func(10)`など)に
            // 戻り値のレジスタを返せるよう、生成済みとして記録する
            self.emitted_calls.push(node_idx);
        }
        Ok(())
    }

    /// 関数の最後に`ret`が無い場合に、`leave; ret`を付ける
    ///
    /// `_start`はOSから直接呼ばれるエントリーポイントで、`ret`で戻る先が
    /// 無い(終了は`exit`のsyscallで行う)ため、`leave; ret`は付けない
    fn gen_implicit_ret(&mut self, fn_name: &str) {
        if fn_name != "_start" && !self.asm_text.ends_with("ret\n") {
            self.asm_text.push_str(CALLEE_RESTORE_MARK);
            self.asm_text.push_str("leave\nret\n");
        }
    }

    /// 使った`callee_saved`のレジスタを、入口で退避・出口で復元する
    ///
    /// 本体に置いた目印(`CALLEE_SAVE_MARK`/`CALLEE_RESTORE_MARK`)を、
    /// 実際に使ったレジスタの`push`/`pop`に置き換える。
    /// `prologue_end`は、スタックフレームの直後(=本体の先頭)の位置
    fn fix_callee_saved(&mut self, prologue_end: usize) -> Result<(), err::ErrKind> {
        let callee_regs = self.used_reg.callee_regs_to_save();

        let fmt_reg_fn = |reg: &asm_gen::RegisterMode| -> Result<String, err::ErrKind> {
            let r = self.asm_fmt.get_fmt_reg(reg.unwrap_normal(), &Size::DQ)?;
            Ok(r)
        };

        let caller_name: Vec<String> = callee_regs
            .iter()
            .map(fmt_reg_fn)
            .collect::<Result<Vec<String>, _>>()?;
        let save_asm = caller_name
            .iter()
            .map(|name| self.asm_fmt.get_push(name))
            .collect::<String>();

        let callee_names: Vec<String> = callee_regs
            .iter()
            .rev()
            .map(fmt_reg_fn)
            .collect::<Result<Vec<String>, _>>()?;
        let restore_asm = callee_names
            .iter()
            .map(|name| self.asm_fmt.get_pop(name))
            .collect::<String>();

        // スタックフレームの作り直し(`build_fn_process`の`replace_range`)は
        // `prologue_end`より前の範囲だけを書き換えるので、先に本体側の目印を置き換えておく
        let body = self.asm_text.split_off(prologue_end);
        let body = body
            .replace(CALLEE_SAVE_MARK, &save_asm)
            .replace(CALLEE_RESTORE_MARK, &restore_asm);
        self.asm_text.push_str(&body);
        Ok(())
    }

    /// 関数の先頭のスタックフレームのアセンブリを生成する
    ///
    /// - `stk_size != 0`: `stk_size`をアライメントした分だけ確保する
    /// - `stk_size == 0`: 確保せずフレームだけ作る(`_start`は作らない)
    fn gen_fn_prologue(&self, fn_name: &str, stk_size: usize) -> String {
        if stk_size != 0 {
            self.asm_fmt.gen_stack_frame(stk_size)
        } else if fn_name != "_start" {
            self.asm_fmt.func_frame_fmt()
        } else {
            String::new()
        }
    }
}

impl AsmEmitter {
    /// オペランドのテキストが、レジスタ`reg`(どのサイズの名前でも)を
    /// 読み出しに使っているか(`(%rbx)`のようなメモリ参照も含む)
    pub(super) fn text_reads_reg(
        &self,
        text: &str,
        reg: RegisterMode,
    ) -> Result<bool, err::ErrKind> {
        Ok(self
            .asm_fmt
            .all_reg_names()?
            .iter()
            .any(|(num, name)| *num == reg && text.contains(name.as_str())))
    }

    #[inline(always)]
    pub(super) fn expect_jmp(&mut self, name: &str) {
        // 次のフォーマットに使うラベルの名前を予約する
        // if self.reserved_label_name.is_none() {
        self.reserved_label_name = Some(name.to_string());
        // } else {
        //     // 予約するラベルの名前は予約するときにNoneで無ければいけない
        //     panic!("system err");
        // }
    }

    #[inline(always)]
    pub(super) fn gen_str_asm(&mut self, dst: usize, value: &str) {
        let label_name = format!("M{}", self.data_idx.to_string());
        let fmt_data = self.asm_fmt.get_str_fmt(&value, &label_name);
        self.data_sec_text.push_str(&fmt_data);
        self.data_map.push((dst, label_name));
        self.data_idx += 1;
    }

    #[inline(always)]
    fn gen_float_asm(&mut self, dst: usize, value: &str, size: &types::Size) {
        let label_name = format!("M{}", self.data_idx);
        let fmt_data = self.asm_fmt.get_float_fmt(value, &label_name, size);
        self.data_sec_text.push_str(&fmt_data);
        self.data_map.push((dst, label_name));
        self.data_idx += 1;
    }

    #[inline(always)]
    pub(super) fn gen_expr_asm(&mut self, expr: &inst::ExprInst) -> Result<(), err::ErrKind> {
        // 三項演算子(`asm_emitter/ternary.rs`)
        // - 条件(`Ternary<比較>`): `cmp`だけを出す。結果のレジスタは作らない
        // - 選ぶ命令(`Ternary`): `mov`と`cmov`で左辺か右辺を結果のレジスタへ置く
        if Self::is_ternary_cmp_kind(&expr.kind) {
            return self.gen_ternary_cmp_asm(expr);
        }
        if expr.kind == inst::ExprKind::Ternary {
            return self.gen_ternary_select_asm(expr);
        }
        // `Set<比較>`: `cmp` + `setcc`で0/1を結果のレジスタへ置く(`asm_emitter/set_cc.rs`)
        if Self::is_set_cc_kind(&expr.kind) {
            return self.gen_set_cc_asm(expr);
        }
        // 結果を置くレジスタを、使用中のレジスタ(引数・変数・他の式の
        // 結果)と`%rax`/`%rdx`を避けて確保する。
        // (以前は`reg_idx`を進めるだけだったため、引数の`%rdi`などまで
        //  上書きしていた)
        let size = self.get_expr_ty(expr.ls);
        let dst = self.alloc_reg(&size);
        self.reg_idx = dst.index();

        let asm = self.format_expr_inst(&expr)?;

        self.asm_text.push_str(&asm);

        // 式の結果を置いたレジスタを使用中として記録する
        self.used_reg.mark_used(&dst);
        self.last_inst_idx.push((expr.dst, dst));

        // この式が使い終わった、オペランドの一時的な結果のレジスタを解放する
        self.release_expr_temp(expr.ls);
        self.release_expr_temp(expr.rs);
        Ok(())
    }

    #[inline(always)]
    pub(super) fn gen_complie_asm(
        &mut self,
        name: &str,
        lines: &[(String, Vec<usize>)],
        asm_fmt_name: Option<&str>,
    ) {
        if let Some(asm_name) = asm_fmt_name {
            if name == asm_name {
                self.deploy_inline_asm(name, &lines);
            } else {
                panic!("unmatch asm name");
            }
        } else {
            self.deploy_inline_asm(name, &lines);
        }
    }

    #[inline(always)]
    pub(super) fn gen_assign_var_asm(
        &mut self,
        name: &str,
        dst: usize,
        value: usize,
        this_is_self: bool,
    ) -> Result<(), err::ErrKind> {
        let is_mem_write = matches!(
            self.curr_inst[dst],
            inst::Inst::Pointer(..) | inst::Inst::InsertArr { .. } | inst::Inst::RefStruct { .. }
        );

        if is_mem_write {
            self.write_mem(name, dst, value, Some(&self.get_var_ty(name)?));
            Ok(())
        } else {
            // 通常の変数への再代入(`b = 10`など)
            self.update_value_info(&name, value);

            // レジスタに置かれている変数は、そのレジスタへそのまま書き込む。
            // (以前は`reg_idx`のレジスタへ変数を「引っ越し」させていたため、
            //  引数や他の変数のレジスタを壊したり、ループの先頭が
            //  参照するレジスタと食い違ったりしていた)
            let var_ty = self.get_var_ty(name)?;
            let current_reg = self.var_reg(name).unwrap_or_else(|| {
                if var_ty.is_float() {
                    RegisterMode::Float(self.reg_idx)
                } else {
                    RegisterMode::Normal(self.reg_idx)
                }
            });
            if matches!(
                self.curr_inst[value],
                inst::Inst::Expr(..) | inst::Inst::Cast { .. }
            ) && self
                .last_inst_idx
                .iter()
                .any(|(idx, reg)| *idx == value && *reg == current_reg)
            {
                return Ok(());
            }
            let s = if this_is_self {
                None
            } else {
                var_ty.wrap_dst_size()
            };

            // 代入先の変数の型(サイズ)を確認し、ポインタ型
            // であれば、専用のフォーマット(`get_ptr`)で
            // アドレスのオペランドを組み立てる
            let text = if var_ty.is_pointer().is_some() {
                self.assign_val_ty_is_ptr(current_reg, value, s.as_ref())?
            } else if var_ty.is_float() {
                self.assign_val_is_float(current_reg, value, &var_ty, s.as_ref())?
            } else {
                self.assign_val_is_not_ptr(current_reg, value, s.as_ref())?
            };

            self.asm_text.push_str(&text);
            // 代入する値が式の結果だった場合、そのレジスタはもう不要
            self.release_expr_temp(value);
            Ok(())
        }
    }

    #[inline(always)]
    pub(super) fn gen_ret_asm(
        &mut self,
        fn_ret_ty: SelfPtrInfo,
        idx: usize,
        fn_name: &str,
    ) -> Result<(), err::ErrKind> {
        // `_start`はOSから直接呼ばれるエントリーポイントであり、
        // `call`で呼ばれたわけではないため`ret`で戻ることができない。
        // `_start`の中に明示的な`return`(`Inst::Ret`)が書かれていた
        // 場合も、通常の`leave; ret`ではなく`sys_exit`で終了させる。
        let _is_start = fn_name == "_start";

        if fn_ret_ty.is_none() {
            if let Some(struct_idx) = self.resolve_struct_idx(idx) {
                let mem = match self.inst_at(struct_idx) {
                    inst::Inst::Struct { mem, .. } => mem,
                    _ => panic!("構造体の戻り値を解決できません"),
                };
                let t = self.emit_struct_init_asm(mem, true)?;
                self.asm_text.push_str(t.as_str());
                // if is_start {
                //     self.asm_text.push_str(
                //         &self.gen_exit_syscall_asm()
                //     );
                // } else {
                if !_is_start {
                    self.asm_text.push_str(CALLEE_RESTORE_MARK);
                }
                self.asm_text
                    .push_str(self.asm_fmt.func_frame_end().as_str());
                self.asm_text.push_str("ret\n");
                // }
                return Ok(());
            }
        }
        let mnemonic = if self.curr_inst[idx].is_float(&self.curr_inst) {
            "float_mov"
        } else {
            "mov"
        };
        let ret_asm = self.format_line(mnemonic, Some(0), idx, None, fn_ret_ty)?;
        self.asm_text.push_str(&ret_asm);
        // if is_start {
        //     self.asm_text
        //         .push_str(
        //             &self.gen_exit_syscall_asm()
        //         );
        // } else {
        if !_is_start {
            self.asm_text.push_str(CALLEE_RESTORE_MARK);
        }
        self.asm_text
            .push_str(self.asm_fmt.func_frame_end().as_str());
        self.asm_text.push_str("ret\n");
        // }
        Ok(())
    }
}
