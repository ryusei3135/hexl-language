use super::*;

impl AsmEmitter {
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
    pub(in crate::compiler::asm_gen) fn build_fn_process(
        &mut self,
        fn_meta_data: &mut (String, def_tree::FnDefInfo),
        asm_fmt_name: Option<&str>,
    ) {
        let this_is_self = fn_meta_data.1.first_param_is_self();
        let fn_ret_ty = fn_meta_data.1.get_ret_ty();
        // 新しく関数の作成、
        let fn_label = Self::fn_label_of(&fn_meta_data.1.name, &fn_meta_data.1.temp_ty);
        self.asm_text
            .push_str(&self.asm_fmt.get_label_fmt(&fn_label));
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

        self.fn_state.curr_inst = mem::take(&mut fn_meta_data.1.body);
        // 構造体の初期化が`self`のポインタ(`%rdi`など)を使い、かつ関数を呼ぶ場合、
        // 呼び出しの引数で上書きされる前に、ポインタを退避する
        if fn_meta_data.0 != "_start" && self.needs_self_ptr_save() {
            let save = self.save_self_ptr_asm();
            self.asm_text.push_str(&save);
        }
        let returned_struct_idx = self.find_returned_struct_idx(fn_ret_ty.is_none());
        let ctx = FnGenCtx {
            fn_name: &fn_meta_data.0,
            fn_ret_ty: fn_ret_ty.as_ref(),
            this_is_self,
            returned_struct_idx,
            asm_fmt_name: &asm_fmt_name,
        };
        // 変数の最後の使用のノード(`last_use.rs`)。そのノードを生成した後で
        // 変数のレジスタを解放する
        let last_uses = Self::var_last_uses(&self.fn_state.curr_inst);
        // ノードの生成でエラーが返ってきたら、`err_pool`へ入れて次のノードへ進む
        // (1つの関数の中の、独立した複数のエラーをまとめて報告するため)
        for (node_idx, node) in self.fn_state.curr_inst.clone().iter().enumerate() {
            // ノードをアセンブリ言語に変換
            if let Err(err_node) = self.gen_node_asm(node_idx, node, &ctx) {
                self.err_pool.borrow_mut().push_err_node(err_node);
            }
            // これ以降使わない変数のレジスタは解放する
            self.release_dead_vars(node_idx, &last_uses);
        }
        self.gen_implicit_ret(ctx.fn_name);
        if let Err(e) = self.fix_callee_saved(prologue_end) {
            (*self.err_pool.borrow_mut()).push_err_node(e);
        }

        // === スタックフレームで確保するサイズを確定させる ===
        // IRが見積もったサイズ(`stk_size`)と、本体の生成で実際に数えた
        // スタックの使用量(`stk_use_counter`)の大きい方を使う。
        // `gen_stack_frame`が8/16byte境界へ切り上げる
        let used_stk_size = self.fn_state.stk_use_counter.max(ir_stk_size);
        let prologue = self.gen_fn_prologue(ctx.fn_name, used_stk_size);
        self.asm_text
            .replace_range(prologue_start..prologue_end, &prologue);
    }

    /// 関数ごとの状態を初期化し、変数へレジスタを割り当てる
    fn begin_fn_state(&mut self, fn_def: &def_tree::FnDefInfo) {
        // 前の関数の変数・レジスタ・スタックの使用量などの状態を持ち越さない
        // (持ち越すとローカル変数のオフセットなどが正しく計算できない)
        self.reset_fn_state();
        // 引数のレジスタと名前を登録(レジスタに収まらない引数はスタックから受け取る)
        self.setting_args_reg_alloc(&fn_def.args);
        // 変数の使用頻度に合わせてレジスタを割り当てる(`reg_mnger.rs`)
        // この関数自身の引数のレジスタには、まだ引数の値が入っているので使わない
        let param_regs: Vec<RegisterMode> = fn_def
            .args
            .iter()
            .enumerate()
            .filter(|(idx, _)| self.param_in_reg(*idx))
            .map(|(idx, arg)| {
                self.arg_register_mode(idx, &arg.ty.clone().try_into().unwrap_or(Size::DQ))
            })
            .collect();
        // 構造体の変数(`s: Name = Name { .. }`)はスタックに置くので、
        // レジスタの割り当てから外し、次の変数に割り当てる
        let mut stack_vars = reg_mnger::UsedRegManager::stack_var_names(&fn_def.body);
        // レジスタに収まらない引数は、スタックの専用の領域へ受け取るので、レジスタを割り当てない
        stack_vars.extend(
            fn_def
                .args
                .iter()
                .enumerate()
                .filter(|(idx, _)| !self.param_in_reg(*idx))
                .map(|(_, arg)| arg.name.to_string()),
        );
        self.used_reg.allocation_register(
            &fn_def.var_counter,
            &param_regs,
            &stack_vars,
            &fn_def.body,
        );
        // 使用回数が0回の変数は、アセンブリを生成しない
        self.fn_state.unused_var_nodes =
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
                        } if other_name == name => self.fn_state.unused_var_nodes.contains(&i),
                        _ => true,
                    });
                if self.fn_state.unused_var_nodes.contains(&idx) && all_removed {
                    self.used_reg.release_allocation(name);
                }
            }
        }
    }

    pub(super) fn arg_register_mode(&self, index: usize, arg_ty: &Size) -> RegisterMode {
        let reg = self.asm_fmt.get_fmt_param::<usize>(index, &arg_ty).unwrap();
        if arg_ty.is_float() {
            RegisterMode::Float(reg)
        } else {
            RegisterMode::Normal(reg)
        }
    }

    /// `index`番目の引数が、引数レジスタで渡されるか(`false`ならスタックで渡される)
    pub(super) fn param_in_reg(&self, index: usize) -> bool {
        index < self.asm_fmt.param_reg_count()
    }

    /// スタックで渡された`index`番目の引数の、呼ばれた側から見たメモリ参照
    ///
    /// `push %rbp; mov %rsp, %rbp`の後なので、`8(%rbp)`が戻りアドレス、
    /// `16(%rbp)`がレジスタに収まらなかった最初の引数、以降8byteずつ上に並ぶ
    /// (呼び出し側が最後の引数から順に`push`するため。`emit_call_func`)
    pub(super) fn stack_param_ref(&self, index: usize) -> String {
        let nth = index - self.asm_fmt.param_reg_count();
        self.asm_fmt
            .fmt_ref_operand_up(FRAME_BASE_REG, STACK_PARAM_BASE + 8 * nth)
    }

    /// 関数の引数を名前とセットでレジスタを登録
    /// 引数が構造体などなら必ず64bitのサイズになる
    /// (レジスタに収まらない引数は対象外。`gen_param_asm`がスタックの値を受け取る)
    fn setting_args_reg_alloc(&mut self, args_def: &[ast::ArgsNode]) {
        args_def.iter().enumerate().for_each(|(idx, arg)| {
            if !self.param_in_reg(idx) {
                return;
            }
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
        self.fn_state.curr_inst.iter().find_map(|node| match node {
            inst::Inst::Ret(idx) => self.resolve_struct_idx(*idx),
            _ => None,
        })
    }

    /// 関数の最後に`ret`が無い場合に、`leave; ret`を付ける
    ///
    /// `_start`はOSから直接呼ばれるエントリーポイントで、`ret`で戻る先が
    /// 無い(終了は`exit`のsyscallで行う)ため、`leave; ret`は付けない
    fn gen_implicit_ret(&mut self, fn_name: &str) {
        if fn_name != "_start" && !self.asm_text.ends_with(self.asm_fmt.get_ret().as_str()) {
            self.asm_text.push_str(CALLEE_RESTORE_MARK);
            self.asm_text.push_str(&self.asm_fmt.func_frame_end());
            self.asm_text.push_str(&self.asm_fmt.get_ret());
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
        // `push %rbp`の直後の`%rsp`は16byte境界にあり、フレームも16の倍数で確保する。
        // `callee_saved`の`push`が奇数個だと、以降の`call`の時点で8byteずれるので、
        // 奇数個のときは8byteの余白を足す(復元では先に解放する)
        let needs_pad = callee_regs.len() % 2 == 1;
        let mut save_asm = caller_name
            .iter()
            .map(|name| self.asm_fmt.get_push(name))
            .collect::<String>();
        if needs_pad {
            save_asm.push_str(&self.asm_fmt.get_stack_alloc_8());
        }

        let callee_names: Vec<String> = callee_regs
            .iter()
            .rev()
            .map(fmt_reg_fn)
            .collect::<Result<Vec<String>, _>>()?;
        let mut restore_asm = String::new();
        if needs_pad {
            restore_asm.push_str(&self.asm_fmt.get_stack_free_8());
        }
        restore_asm.push_str(
            &callee_names
                .iter()
                .map(|name| self.asm_fmt.get_pop(name))
                .collect::<String>(),
        );

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
            let frame = self.asm_fmt.gen_stack_frame(stk_size);
            if fn_name == "_start" {
                // `_start`は`call`で呼ばれないので、入口の`%rsp`が既に16byte境界にある。
                // `push %rbp`で8byteずれるので、余白を足して境界に戻す
                format!("{}{}", frame, self.asm_fmt.get_stack_alloc_8())
            } else {
                frame
            }
        } else if fn_name != "_start" {
            self.asm_fmt.func_frame_fmt()
        } else {
            String::new()
        }
    }
}
