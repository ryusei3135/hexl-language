# [asm_emitter] モジュール

このモジュールは、**`gen/asm_emitter.rs`の処理の一部** を担当します。

## 重要なファイル`[insert_fmt_reg.rs]`
生成されるアセンブリ言語のフォーマットファイル
に埋め込まれる`%{1}`などのサイズ指定がないけど特定のレジスタ
を埋め込む処理、これを停会するAPIを提供


## 開発者向けの注意点・メモ
* このモジュールは、`gen/asm_emitter.rs`のみ使用可能

- `[operand_txt.rs]`について
    `param_ref`関数のレジスタサイズは、ポインタなら64bit・それ以外は
    変数自身の型のサイズを使うように修正済み(`param_ref`関数のレジスタ
    サイズが固定になっていた`ver 0.8.9`の不具合は解消済み)
- `2026-09-12`ニーモニックのサイズをすべて生成するように修正中

- `2026-10-03`operand_txt.rsのinsert_arr_txtで構造体を配列のindexにする処理

### fmt_one_expr_mnemo_resize
- `operand_txt.rs`
- ニーモニックのサイズを調整
- `2026-10-03`配列の添字が構造体メンバーの場合(`[arr a.a else 0] = 0`)、`arr.rs`の`ref_arr_for_struct_member`が添字付きアドレッシング(`-offset(%rbp, %rcx, 要素サイズ)`)を生成するように修正。要素`i`は`%rbp - (arr_base + (i+1)*要素サイズ)`に置かれる(`VarIndexInfo.arr_base`)ため、添字を`neg`してから参照する
- `2026-10-03`関数先頭のスタックフレームの確保サイズを、本体の生成中に数えた使用量(`stk_use_counter`)とIRの`stk_size`の大きい方にし、16byteへアライメントして`call_func.rs`の`build_fn_process`で作り直すように修正
- `2026-10-03`構造体をコンストラクタで初期化する際、暗黙の`self`のスタック領域(`GetPtr`)をIRの`stk`ではなく`stk_use_counter`から確保する(`alloc_struct_stk`)ように修正。配列と領域が重なるバグの修正
- `2026-10-03`構造体のメンバーの並びを変更。ポインタは先頭のメンバーを指し、`(%rdi)`、`-4(%rdi)`のように`pos - メンバーのサイズ`をオフセットにする(`ref_struct_txt`/`emit_struct_ini_asm`/`alloc_struct_stk`)
- `2026-10-03`配列の並びを変更。要素0が最もアドレスが小さく、要素`i`は`%rbp - arr_base + i*サイズ`(`arr_base`は要素0のオフセット)。添字が構造体のメンバーの場合は`neg`ではなく`movslq`で符号拡張して`-arr_base(%rbp, %rcx, サイズ)`で参照する
- `2026-10-03`配列などメモリに実体がある変数(`VarIndexInfo.in_mem`)を、レジスタの使用中の記録とインラインアセンブラのレジスタ退避の対象から外した(`#asm`の前に`movl %eax, %ecx`が出るバグ)
- `2026-10-03`配列の並びを元に戻した。要素`i`は`%rbp - (arr_base + (i+1)*サイズ)`(`-4`, `-8`, ...)で、添字が構造体のメンバーの場合は`movslq`で符号拡張した後に`negq`してから`-(arr_base+サイズ)(%rbp, %rcx, サイズ)`で参照する
- `2026-10-03`配列の添字が構造体のメンバーの場合、`movslq`→`negq`→`addq $(要素数-1)`で添字を`要素数-1-添字`に反転し、最後の要素の位置`-(arr_base+要素数*サイズ)(%rbp, %rcx, サイズ)`を基準に参照するように変更

- `2026-10-10`ポインタでない構造体の変数(`a: Name = Name{..}`など)は、`VarIndexInfo`にレジスタ番号ではなく`%rbp`からのオフセットで保存するように修正
    - 場所: `asm_emitter.rs`の`VarIndexInfo`(`offset`フィールド・`new_stack`・`NO_REG`)
    - `is_stack == true`の変数は、`reg`が`VarIndexInfo::NO_REG`(`usize::MAX`)で、位置は`offset`(構造体先頭のメンバーの`%rbp`からのバイト数)だけが持つ。以前は`reg`にオフセットを入れていたため、レジスタ番号と取り違えて`get_fmt_reg`が範囲外でpanicしていた
    - 登録する場所: `call_func/mem_ir.rs`の`mov_value_ir`(コンストラクタ呼び出しで初期化)と`struct_mem`(`Name {..}`リテラルで初期化)
    - `offset`を読む場所: `access.rs`の`var_operand`、`operand_txt.rs`の`ref_struct_txt`、`inline_asm.rs`の`extract_operand_text_sized`(`AssignVar`)
    - 新しく変数の位置を読むコードは、`is_stack`を確認して`offset`を使うこと(ポインタの変数は従来どおり`reg`)
- `2026-10-10`変数へのレジスタの割り当て(`reg_mnger.rs`)
    - 関数のアセンブリを作る前(`call_func.rs`の`build_fn_process`)に、IRが数えた変数の使用回数(`FnDefInfo.var_counter`)の多い順に、`allocation_register`でレジスタを割り当てる。`callee_saved`(`rbx`/`r12`〜`r15`)を先に、次に`caller_saved`を使う。`%rax`/`%rdx`(`RESERVED_REGS`)と、その関数自身の引数のレジスタは使わない
    - 変数を定義するとき(`call_func/mem_ir.rs`の`mov_value_ir`)は、割り当てたレジスタが空いていればそれを自動的に使う。空いていなければ従来どおり`alloc_reg`で探す
    - 割り当てたレジスタは、変数が定義される(`bind_var`)まで取り置かれ、`alloc_reg`は使わない(`is_reserved`)
    - スコープ(反復処理や関数内のブロック)が終わる(`EndScope`)と、その中で使用中になったレジスタは全て解放され(`end_scope`)、他の変数や式が使える。外側の変数のレジスタは残す(`end_scope_asm`)。関数が終わると割り当て自体を破棄する(`clear`)
    - 使った`callee_saved`のレジスタは、関数の入口で`push`、`ret`の直前で`pop`する。使うレジスタは本体を生成し終わるまで分からないので、目印(`{callee_save}`/`{callee_restore}`)を置いて`build_fn_process`の最後に置き換える。`_start`は戻り先が無いので対象外
- `2026-10-11`スタックに置く変数(ポインタでない構造体の変数`s: Name = Name {..}`)を、レジスタの割り当てから外した
    - 以前は、この変数が`VarCounter`に出てくると`allocation_register`がレジスタを割り当てて取り置き(`pending`)にしていた。構造体の変数はレジスタを持たず`bind_var`も呼ばれないため、そのレジスタが解放されず、次の変数に使えなかった
    - `reg_mnger.rs`の`UsedRegManager::stack_var_names`が、関数のIR(`Inst::Mov`の`size`が`Struct`で`name`がある)から名前を集め、`allocation_register`の`stack_vars`に渡す。該当する変数は飛ばし、次の変数にレジスタを割り当てる(`call_func.rs`の`build_fn_process`)
    - 配列(`Inst::MemoryValue`)も同じくレジスタを持たないが、今回は対象外

