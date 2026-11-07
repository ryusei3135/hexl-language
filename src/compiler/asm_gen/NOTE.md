# 重要なAPI

## メモ
- reg_mngerでレジスタの管理の方法をあとで修正

## アセンブリ言語
- アセンブリ言語のテキストのレジスタのサイズを取得する方法
    - [場所] `src/gen/mng_fmt/`
    - [名前] get_reg_size

## 重要
### get_expr_ty
- `asm_emitter.rs`
- irの式をたどって自分の式の型を取得する関数

### mnemonic_size
- アセンブリフォーマットの `fmt.mnemonic_size` で、メモリ読み書き以外の
    命令にサイズ接尾辞を付けるかを指定する。
- `true`: `movq`/`addl` のようにサイズを付ける(AT&T構文向け)。
- `false`: `mov`/`add` のようにサイズを付けない(Intel構文向け)。
- メモリを読み書きする命令は、設定値に関係なくサイズ接尾辞を付ける。
- `call`/`ret`/`jmp`/`push`/`pop` のように命令仕様上サイズ接尾辞を
    持たない命令は対象外とする。
- 新しいアセンブリフォーマットでは `fmt.mnemonic_size` を指定する。
    未指定の場合は `true` として扱われる。

### ensure_mnemonic_size (2026-10-23)
- `asm_fmt/mng_fmt.rs`の`MngAsmFmt::ensure_mnemonic_size`。`to_asm_text`が出力する**すべての命令**を最後にこの関数へ通し、
    サイズ接尾辞(gasの`b`/`w`/`l`/`q`)の無い整数命令に必ず付ける(`mnemonic_size`の設定に関係なく常に付く)。
    テンプレートを直接埋めただけの行(`sub $16, %rsp`、`push`/`pop`など)の付け忘れもここで補う。
- サイズは最後のレジスタオペランドの幅で決める。`lea`/`push`/`pop`は常に`q`。
    すでに付いている命令・SSE・`call`/`jmp`/`ret`・ラベル・ディレクティブは変えない。
- `#asm`ブロックのユーザーが書いた行は、行末の`RAW_ASM_MARK`で変更の対象外にする(`inline_asm.rs`)。
- レジスタが無く幅を決められない行(`mov $1, -4(%rbp)`など)は変えない。そのような命令は生成時に
    `mov_line`/`tmpl_line`でサイズを付けること。

## アクセス用の補助関数
### access.rs
- 変数の表(`var_hash_map`)・IR(`curr_inst`)・静的領域(`data_map`)・
    アセンブリのフォーマッタ(`asm_fmt`)へのアクセスをまとめたモジュール。
    新しいコードでは、`self.var_hash_map.get(..)`や
    `get_opcode_tmpl(..).replace(..)`を直接書かずに、ここの関数を使う。
- 変数: `var_info` / `find_var_info` / `var_info_mut` / `var_size` /
    `var_reg` / `var_operand` / `arr_elem_ty` / `is_reg_held_by_var`
- 構造体: `struct_member_offset`(メンバーのオフセット) /
    `self_ptr_reg`(暗黙の`self`ポインタのレジスタ)
- 静的領域: `static_label`
- メモリ参照: `rbp_ref` / `ref_base_offset` / `reg64` / `ptr_deref_operand`
- 命令の1行: `fill_tmpl`(テンプレートの穴埋めのみ) /
    `tmpl_line`(ニーモニックのサイズ調整まで) / `mov_line`
- IRのノード: `inst_at`(`curr_inst[idx].clone()`)

## リファクタリングの記録(2026-10-11)
- 長い関数を役割ごとの関数に分けた(生成するアセンブリは変えていない)。
- 分けた関数の一覧と処理の流れは、各モジュールのNOTE.mdに記載している。
    - `call_func/NOTE.md`: `build_fn_process` / `emit_call_func` / `mov_value_ir` / `mem_val_ir`
    - `asm_emitter/NOTE.md`: `ref_arr_for_struct_member` / `extract_operand_text` / `format_line` / `format_expr_inst`

## 符号付き/符号なし整数(2026-10-13)
- `types::Size`の`DB/DW/DD/DQ`(符号付き)と`UDB/UDW/UDD/UDQ`(符号なし)を区別して生成する。
- レジスタの選択・ニーモニックのサイズ接尾辞は「幅」だけで決まるので、符号なしは同じ幅の符号付きと同じ扱い
    (`mng_fmt.rs`の`get_fmt_reg` / `fmt_mnemonic_resize` / `get_fmt_struct_member`、`asm_emitter.rs`の`value_reg_size`)。
- 符号の有無で出力が変わる処理(判定は`sign.rs`の`SizeSign::is_unsigned`):

    | 処理 | 符号付き | 符号なし | 場所 |
    |------|----------|----------|------|
    | `<` `>` | `cmp_l`/`cmp_g`(`jl`/`jg`) | `cmp_b`/`cmp_a`(`jb`/`ja`) | `format_expr_inst` / `get_unsigned_cmp_tmpl` |
    | `/` `%` | `cltd`/`cqto` + `idiv` | `xor %edx, %edx` + `div` | `div_ir.rs` |
    | 構造体メンバーの添字 | `movs?q` | `movz?q`(32bitは`movl`のみ) | `arr.rs`の`load_index_reg` / `get_zero_extend` |

- `+` `-` `*`(2オペランドの`imul`)と`==` `!=`は符号に関係なく共通。
- 比較のどちらか一方のオペランドが符号なしなら、符号なしとして扱う。
- アセンブリのフォーマット(`asm_fmts/gcc_x64.yaml` / `asm_fmt/x64.rs`)に、`cmp_b` / `cmp_a` / `movzbq` / `movzwq`を追加した。
    フォーマットに定義がない場合は、`cmp_l`/`cmp_g`の`jl`/`jg`を`jb`/`ja`に置き換えるか、AT&T形式の
    テンプレートを使うので、古いフォーマットファイルでも動く。

## 呼ばれる関数のみ生成(2026-10-15)
- `_start`以外の関数は、`call_func.rs`の`emit_call_func`が呼び出しを生成するときに`build_fn_process`で生成する(`emit_callee_fn`)。
    呼ばれない関数は生成されない。
- `to_asm_text`は、`_start`と`global_funcs`(他のファイルから呼ばれる)だけを直接生成し、残りは`pending_funcs`(ラベル -> 定義)へ登録する。
- `emit_call_func`は、呼ぶ関数のラベルが`pending_funcs`にあれば取り出して`emit_callee_fn`で生成する。取り出してから生成するので、再帰呼び出しで二重に生成されない。
- `emit_callee_fn`は、呼んだ側の生成中の状態(`asm_text`/変数/レジスタ/`curr_inst`/スタック使用量など)を退避してから`build_fn_process`を呼び、終わったら戻す。
    呼ばれた側のアセンブリは`fn_asm_out`へ溜め、`to_asm_text`の最後に`_start`の後ろへ結合する(呼んだ側の関数の途中に挟まらないため)。
- 関数ごとの状態のフィールドは`FnState`(`fn_state.rs`)に追加する。`emit_callee_fn`が`FnState`を丸ごと退避・復元するので、個別に加える必要は無い(2026-11-07〜)。
    ただし`used_reg`は`AsmEmitter`のフィールドのままで、`clone`で退避している。

## 使用回数が0回の変数は生成しない(2026-10-15)
- `reg_mnger.rs`の`UsedRegManager::unused_var_nodes`が、生成しない変数を定義する`Inst::Mov`のノードidを集める
    (`call_func.rs`の`begin_fn_state`)。`gen_node_asm`は、そのノードのアセンブリを生成しない。
    `allocation_register`も、使用回数0回の変数にはレジスタを割り当てない。
- 生成しない変数
    1. `VarCounter`(`FnDefInfo.var_counter`)の使用回数が0回(出てこない場合も0回)の変数
    2. 生成しない変数の初期値の式の中で使われている変数のうち、使用回数のすべてがその式の中にあるもの。
        例: `a`が0回で`a: int = b + 1`のとき、`b`が1回(=この式の中の1回)だけなら`b`も生成しない。
        生成しない変数が増えなくなるまで繰り返すので、`c`→`b`→`a`と使われていれば、`a`→`b`→`c`の順に全て消える
        (`collect_pure_expr_vars`が式の中の変数を集める)。
- 生成しない変数の初期値の式(`Inst::Expr`)と、その中の`Inst::Float`/`Inst::Str`も生成しない。
    `unused_var_nodes`が返す集合にこれらのノードidも含め、`gen_node_asm`が飛ばす
    (`fv3: float = fv + f2`で`fv3`が未使用なら、`fv`・`f2`・`fv + f2`の命令・`.data`の定数も消える)。
    以前は式を含む初期値を対象外にしていたため、`fv`/`f2`が残るバグがあった(2026-10-16修正)。
- 対象外: 初期値の式に関数の呼び出し/代入/inlineアセンブラがある変数(副作用が消えるため)、
    再代入(`Inst::AssignVar`)がある変数。
- `FnState::unused_var_nodes`は関数ごとの状態なので、`FnState`ごと`emit_callee_fn`で退避・復元される。

## 退避した変数は参照されるまで復元しない(2026-10-16)
- 関数呼び出し(`emit_call_func`)で、`caller_saved`のレジスタに置かれた**生きている変数**は、`push`/`pop`ではなく
    スタックフレームの専用の領域(`%rbp`相対、レジスタごとに1つ。`stk_use_counter`から確保)へ`mov`で退避し、
    呼び出しの後は復元しない(`lazy_spill.rs`)。式の一時的な結果のレジスタは従来どおり`push`/`pop`。
- 退避中のレジスタは`spilled_regs`に持ち、使用中のまま(他の値に使わない)にする。中身は壊れているので、次の呼び出しでも保存しない。
- ノードの生成前(`gen_node_asm`の`prepare_node_spills`)に、そのノードの引数のIRをたどって参照する変数を集め
    (`collect_node_refs`)、退避中なら復元する。たどれないノード(構造体・バリアント・`MemoryValue`など)は全部復元し、
    そのノードの中の呼び出しは退避したままにしない(`node_vars_all`)。
- 同じノードの中の呼び出しは、そのノードが参照する変数(`node_vars`)だけ、呼び出しの直後に復元する(後続のオペランドで使うため)。
- 制御フローで状態が食い違わないよう、`Block`/`Jmp`/`ExpectJmp`/インラインアセンブラの前に全て復元する。
    `ExpectJmp`の後(条件ジャンプの条件式の中)の呼び出しは退避したままにしない。
- `Ret`の後と、`EndScope`で解放されたレジスタ(スコープが終わった変数、`dead_vars`)の退避は、復元せず捨てる。
- 関数ごとの状態(`spilled_regs`/`spill_slots`/`node_vars`/`node_vars_all`/`dead_vars`)は`FnState`のフィールドで、
    `reset_fn_state`で初期化し、`emit_callee_fn`で退避・復元する。

## レジスタが足りない変数のスタック配置(2026-10-23)
- 変数を定義するとき(`call_func/mem_ir.rs`の`bind_var_reg`)、割り当て済みのレジスタも
    空きレジスタ(`try_alloc_reg`)も無ければ、変数をスタックへ置く(以前は`alloc_reg`がpanicしていた)。
- 式の結果を置くレジスタ(`alloc_reg`)が足りない場合は、生きている値をスタックへ退避してレジスタを再利用する。
    退避した式の結果は`spilled_expr_results`でオフセットを管理し、後続のオペランドとしてスタックから参照する。
- `define_spilled_var`(`asm_emitter/var_mnger.rs`)が、8byteの領域を`stk_use_counter`から確保し、
    初期値を書き込んで、`VarIndexInfo::new_stack(offset, size, dst)`で変数として登録する。
    以降の参照は`var_operand`が`-N(%rbp)`のメモリ参照にする。
- メモリからメモリへの`mov`はできないので、値は一時レジスタを経由して書く。
    - 整数・ポインタ: `%rax`(`RESERVED_REGS`なので変数や式には使われていない)
    - 浮動小数点: `xmm`の最後のレジスタを、前後でスタックへ退避・復元して使う
- 再代入(`gen_assign_var_asm`)も同じ経路(`assign_spilled_var`)でメモリへ書く。
- スタックに置いたポインタ変数の指す先を参照する場合(`p[i]`・`p.member`)は、
    `ptr_var_base`が`%rdx`へ読み込んでからベースにする。
- 構造体の変数(`is_stack`)と区別するときは`is_spilled_scalar`を使う(`Size::Struct`でない`is_stack`)。
- 未対応: インラインアセンブラの`${var}`・引数(`Inst::Param`)に、スタックへ置いた変数を渡すケース。

## アセンブリのテキストは`asm_fmt/x64.rs`から作る(2026-10-25)
- アセンブリの命令・ラベル・ディレクティブ・`%rbp`/`(%rsp)`などのレジスタ名は、生成するコードに直書きせず、
    `asm_fmt/x64.rs`(`GCC_X64`)のテンプレートを`MngAsmFmt`の関数経由で参照する。
- フォーマットファイルに定義が無いキーは、`x64::default_tmpl`(`MngAsmFmt::tmpl_or_default`)で`x64.rs`の定義を使う。
- 追加したキー: `label` / `jmp` / `sp_alloc_16` / `sp_free_16` / `sp_ref` / `cltd` / `cqto` / `cdq` / `cqo` / `zero_reg` /
    `idiv` / `udiv` / `neg` / `add_imm` / `movs{b,w,l}{w,l,q}` / `movz{b,w}{w,l,q}` / `data_long` / `data_float` / `data_double`
- 呼び出す関数: `get_label_fmt` / `get_jmp` / `is_jmp_line` / `get_ret` / `get_stack_alloc_16` / `get_stack_free_16` /
    `get_stack_top_ref` / `get_zero_reg` / `get_div_inst` / `get_sign_extend_acc`。`FRAME_BASE_REG`の定義も`x64.rs`にある。
- `ret`/`jmp`/ラベルは`{space}`付きのテンプレートになったので、出力のインデントが2文字増える(動作は同じ)。

## アセンブラ生成のバグ修正(2026-10-31)
- `call_func.rs`の`caller_save_pair`: 呼び出し前後の`push`/`pop`を同じレジスタ列から同時に作る
    (復元は逆順。以前は`pop`が無効化されていた)。退避が16byteの奇数倍なら`sub $8, %rsp`の余白を挟み、
    `call`の時点で`%rsp`を16byte境界にする。
- `fix_callee_saved`: `callee_saved`の`push`が奇数個なら8byteの余白を足す(復元では先に解放)。
    `_start`はフレームを作るときに余白を足す(入口の`%rsp`は`push %rbp`前から16byte境界)。
- `access.rs`の`fill_tmpl`/`tmpl_line`: `address`(`lea`)のsrcが汎用レジスタなら`mov`にする
    (`leaq %rcx, %rbx`は不正な命令)。
- `inline_asm.rs`: `syscall`の検出を大文字小文字・区切り文字に対応させた(`%rcx`/`%r11`は破壊される扱い)。
- 未修正(要確認): 構造体メンバーのレイアウト(`struct_member_offset`)は、小さいメンバーの後に大きいメンバーが
    続くと領域が重なる。IR側でパディング/オフセットを決める必要がある。

## ポインタ/配列の添字を上向きに変更(2026-10-31)
- 要素`i`は`ptr + i * 要素サイズ`(ポインタ)、`%rbp - arr_base + i * 要素サイズ`(スタック配列)。以前は下向き。
    `malloc`した領域(上向きに使う)と、スタック配列へのポインタの両方で同じ向きになる。
- `arr_base`の意味を「要素0の位置(領域の一番下)」に変更した(`mem_ir.rs`の2か所・`operand_txt.rs`の`init_arr_txt`)。
- `types::Size::Pointer`の`to_bytes()`を、指す先のサイズから8に変更(`types.rs`)。
    構造体のポインタ型メンバーが4バイトと数えられ、`len`のメンバーと重なっていた。
    `ir`側で`Pointer`の`to_bytes()`を「指す先のサイズ」として使っている箇所があれば、`is_pointer()`の`ty`を使うこと。

## 条件付きの反復処理の終了時のレジスタ解放(2026-11-03)
- `asm_emitter.rs`の`release_loop_cond_regs`(`call_func.rs`の`Inst::Block`の処理から呼ぶ)。
- `loop <条件>`の条件の式の結果は`StartScope`より前に確保され誰にも消費されないため、`EndScope`では
    解放されず、反復処理が終わった後も使用中のまま残っていた。
- 反復処理の終了位置(`Jmp(start)`の直後の`Block(end)`)で、条件の式(`Block(start)`直後の`ExpectJmp`から
    最初の`Jmp(end)`まで)の一時的な結果を`release_expr_temp`で解放する。変数が持つレジスタは解放しない。
- 条件のない`loop`、`if`/`match`のラベルは対象外(後ろ向きのジャンプと、先頭直後の`ExpectJmp`で判定)。

## 静的変数がレジスタを占有し続ける不具合の修正(2026-11-03)
- 静的変数(`is_static_var`/`mov_static_var`)は値をラベル(`M0(%rip)`)で参照し、レジスタを持たないが、
    `reg = reg_idx`の通常の変数として登録していた。そのため(1)`insert_var_info`がそのレジスタを使用中にして
    ずっと残り、(2)`is_reg_held_by_var`が同じ番号の一時レジスタ(静的変数のアドレスなど)を「変数が持っている」と
    見なして`release_expr_temp`が解放しなかった。
- `VarIndexInfo::new_static`で`in_mem`の変数として登録し、`VarIndexInfo::holds_reg`
    (`!is_stack && !in_mem`)でレジスタを保持する変数だけを数える。
    `is_reg_held_by_var`/`end_scope_asm`のレジスタの復元/`inline_asm.rs`のレジスタ付け替えが使う。

## 変数の最後の使用でレジスタを解放(2026-11-03)
- `last_use.rs`。`build_fn_process`が、関数のIRから変数ごとの最後の使用のノード(`var_last_uses`)を求め、
    そのノードを生成した後で`release_dead_vars`が、変数を`dead_vars`に入れてレジスタを使用中から外す。
- 以前は、変数のレジスタがスコープ(`_start`なら関数全体)の最後まで使用中のままで、1回しか使わない
    変数(`%rcx`の文字列ポインタなど)が、後続の関数呼び出しのたびに退避・復元されていた。
- 定義より前から続く反復処理の中で最後に使う変数は、反復処理の先頭へ戻る`Jmp`の位置まで延ばす。
- 対象は`Inst::Mov`で定義したローカル変数のみ(引数・スタック・メモリ上の実体・静的領域は対象外)。
- `end_scope_asm`は`dead_vars`の変数のレジスタを戻さない。
- 旧: `locate_del_vars`(`VarCounter`が記録するidは、使われたときの`id_counter`で、値を使うノードより
    前になることがある)と`release_allocation`の呼び出しは、この仕組みに置き換えた(デバッグの`println!`も削除)。

## 条件ジャンプの式の結果レジスタの解放(2026-11-03)
- `gen_expr_asm`: 予約したラベル(`ExpectJmp`)がその式で使われた(=`cmp` + `jz`のような条件ジャンプだけを
    出した)場合、結果のレジスタ(`dst`)は使用中にせず`last_inst_idx`にも記録しない。
    以前は`if`/`match`/`loop`の条件のたびにレジスタが1つ使用中のまま残り、`StartScope`より前に確保される
    ため`EndScope`でも解放されず、以降の関数呼び出しで`pushq %rcx`のように退避され続けていた。
- `loop`の終了位置での条件レジスタの解放(`release_loop_cond_regs`)は、条件ジャンプ以外の式
    (`Set<比較>`など)の結果が残る場合の保険として残す。

## 条件が`a && b`のときに条件ジャンプが出ない不具合の修正(2026-11-03)
- `cond { c >= 0x61 && c <= 0x7A => .. }`の条件は、`SetGreaterThenEq`/`SetLessThenEq`の結果を`AndCalcul`で
    つなぐ。`AndCalcul`のテンプレートには`{label}`が無いため、`ExpectJmp`が予約したラベルが使われず
    条件ジャンプが出なかった(どの条件も不成立になり、必ずelseの処理になる)。
- `gen_expr_asm`: ラベルが予約されたまま使われなかった式が条件の一番外側の式(結果を後ろのノードが
    使わない。`is_expr_consumed`)なら、`cmpq $0, reg` + `jnz <ラベル>`を出す(`emit_jump_if_nonzero`)。
    条件の途中の式(`(a & b) == 0`の`a & b`など)は、後ろの式が使うので対象外。

## `Set<比較>`の結果を8bitのままにする(2026-11-03)
- `asm_emitter/set_cc.rs`: `setcc`の後の`movzbl`(32bit/64bitへの拡張)をやめた。結果は8bitのレジスタ(`%cl`など)のまま。
- `get_expr_ty`は`Set<比較>`の式に`Size::DB`を返す(以前は`ls`の型)。結果を読む式(`a && b`の`andb`、
    `cmpb $0, %dil` + `jnz`など)は8bitのレジスタを使う。
- 注意: 結果の上位ビットは書き換えない(ゴミが残る)ので、8bitより広い型の演算のオペランドにすると、
    広い名前で読んだ上位ビットは不定になる。広げて使う場合は`Cast`(`movzbl`)を通すこと。

## 構造の整理(2026-11-07)
生成するアセンブリは変えていない(ロジックの変更なし。ファイルの分割・状態の集約・重複の削除のみ)。

- **関数ごとの状態を`FnState`へ集約**(`fn_state.rs`)。`AsmEmitter`の関数ごとのフィールド20個
    (`var_hash_map`/`curr_inst`/`stk_use_counter`など)は`self.fn_state.<名前>`でアクセスする。
    - `reset_fn_state`は`FnState::default()`への置き換え + `used_reg.clear()`。
    - `emit_callee_fn`は`mem::take(&mut self.fn_state)`の退避・復元。フィールドの追加時の退避漏れが起きない。
    - 以前の`reset_fn_state`が初期化していなかった`arr_index_temp`も、関数ごとに`None`へ戻るようになった
        (前の関数の値が残るのは不具合になるはずなので、正常なコードでは出力は変わらない)。
- **ファイルの分割**
    - `call_func.rs`(1175行) → `call_func.rs`(呼び出しの生成) + `call_func/`の`caller_save.rs`(callerレジスタの退避/復元) /
        `args.rs`(引数の設定) / `fn_build.rs`(`build_fn_process`・プロローグ・`callee_saved`) /
        `node_asm.rs`(`gen_node_asm`) / `expr_asm.rs`(式・代入・`ret`など)。
    - `asm_emitter.rs` → `asm_emitter.rs`(`new`・`to_asm_text`・基本操作) + `asm_emitter/`の`scope.rs` / `format_line.rs` /
        `node_ty.rs` / `expr_inst.rs`。
    - `reg_mnger.rs` → `reg_mnger/unused_vars.rs`(使用回数0の変数の検出) / `reg_mnger/emitter_regs.rs`(`alloc_reg`など)を分離。
    - `asm_fmt/x64.rs`と`aarch64.rs`で重複していた`Static*`の型と`to_asm_format`を`asm_fmt/static_fmt.rs`へ共通化。
        `x64.rs`の残りは`GCC_X64`の静的なテーブルなので、これ以上は分割していない。
- 分割で、元が`pub(super)`だった関数は`pub(in crate::compiler::asm_gen)`にした(`pub(super)`のままだと可視範囲が狭くなるため)。
    分割した`call_func/`内で呼ぶだけの関数は`pub(super)`。

## 引数レジスタが足りない場合はスタックで渡す(2026-11-07)
引数レジスタの本数(`MngAsmFmt::param_reg_count`、System V なら6本)を超える引数(`param_in_reg(index) == false`)を、
スタックで渡す。以前は`param_fmt[index]`が範囲外になってpanicしていた。

- **呼び出し側**(`call_func.rs`の`emit_call_func` / `call_func/args.rs`の`build_arg_lines`・`push_arg_asm`)
    - 収まらない引数は、**最後の引数から順に`push`**する(System V AMD64 ABI)。引数レジスタへ値を書く前に積む。
    - 積む引数が奇数個なら、先に`sub $8, %rsp`で余白を確保し、`call`の時点で`%rsp`を16byte境界にする
        (callerレジスタの退避は`caller_save_pair`が16byteの倍数にしている)。
    - `call`の後、`add $(8 * 個数 + 余白), %rsp`で解放(`get_stack_free_bytes`。テンプレートのキーは`sp_free_n`)。
        その後でcallerレジスタを復元する。
    - 値は`%rax`経由で`push`する(整数・ポインタ。`"address"`は`lea`してから)。64bitの整数レジスタは直接`push`。
        浮動小数点は`xmm`のレジスタなら`sub $8, %rsp` + `movss/movsd`、メモリ・定数は整数のレジスタ経由でビットをコピーする。
- **呼ばれた側**(`call_func/node_asm.rs`の`gen_param_asm`)
    - `push %rbp; mov %rsp, %rbp`の後、`16(%rbp)`が収まらなかった最初の引数、以降8byteずつ上(`stack_param_ref`)。
    - その値を、この関数のスタックの領域(`stk_use_counter`から確保)へ読み込んで、スタックに置いた変数として登録する
        (`define_spilled_var`)。以降の参照は`-N(%rbp)`になり、再代入・ポインタの参照(`ptr_var_base`)も既存の仕組みで動く。
    - レジスタの割り当てから外す(`begin_fn_state`が`stack_vars`へ加える)。`setting_args_reg_alloc`も登録しない。
- 注意・未対応
    - 構造体を値として渡す7番目以降の引数(`Size::Struct`)は未対応(`is_stack`の構造体の変数と区別できないため)。
    - インラインアセンブラの`${var}`が、スタックで渡されたポインタ型の引数を指す場合は、指す先ではなくポインタを置いた領域を返す。
    - 浮動小数点の引数も、整数と同じ引数の番号(`index`)でレジスタに収まるか決める(従来の`get_fmt_param`の規則のまま)。

## 生成した命令の不正の修正(2026-11-07)
- `call_func/assign_var.rs`の`write_mem`(メモリへの書き込み): 値がレジスタなら、書き込むサイズの名前に直す
    (`movl %rax, ..`→`movl %eax, ..`、`movb %r8, ..`→`movb %r8b, ..`)。値がメモリ(スタックに置いた変数など)なら、
    メモリからメモリへは`mov`できないので`%rax`(書き込み先が`%rax`を使うなら`%rdx`)を経由する。
- `asm_emitter/expr_inst.rs`: 両方のオペランドがメモリのとき、先頭に付ける`mov`(サイズ接尾辞付き)を、
    サイズの調整(`fmt_one_expr_mnemo_resize`)の後で付ける。以前は`mov`がもう一度置き換えられて`movqq`になっていた。
- `asm_fmt/x64.rs`の`mul`/`imul`のテンプレート: `mul {src2}`は即値を取れない(`mul $10`は不正)ので、
    `imul {src2}, %rax`(2オペランド)にした。下位64bitは符号付き・符号なしで同じ。

