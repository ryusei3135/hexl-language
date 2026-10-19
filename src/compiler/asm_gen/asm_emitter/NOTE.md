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


## リファクタリング(2026-10-11)
長かった関数を役割ごとに分けた。**生成されるアセンブリは変えていない**。

### `arr.rs` の`ref_arr_for_struct_member`
配列の添字が構造体のメンバー(`[arr a.a else 0]`)のときの、添字付きアドレッシングを作る。

| 順 | 処理 | 関数 |
|----|------|------|
| 1 | 前回の添字用の一時レジスタを解放 | (本体) |
| 2 | 配列/ポインタの基準(要素サイズ・`arr_base`・ベースのレジスタ・要素数)を調べる | `arr_ref_base`(`ArrRefBase`) |
| 3 | 構造体のメンバーを、添字用のレジスタへ64bitに符号拡張して読み込む | `load_index_reg` |
| 4 | 符号を反転(`neg`)し、配列なら`要素数-1`を足して、参照のオフセットを決める | `adjust_index_reg` |
| 5 | `-offset(%rbp, %reg, 要素サイズ)`を組み立てる | (本体) |

- `load_index_reg`: 8/16bitは`movs?q`でメモリから直接符号拡張、32bitは`movl`→`movslq`、64bitはそのまま`movq`。
- `adjust_index_reg`: ポインタは要素0を指すのでオフセット0、配列は`arr_base + 要素サイズ * 要素数`。

### `asm_emitter.rs`
- `extract_operand_text`(ノードのオペランドの文字列を返す)
    - 関数呼び出しのノード → `call_result_operand`(未生成なら`emit_call_func`で生成し、戻り値のレジスタ`%eax`などを返す)
    - 式などの計算結果のノード → `expr_result_operand`(`last_inst_idx`のレジスタを、式自身の型のサイズで返す)
- `format_line`(1行の命令を作る)は、`src1`の種類で振り分ける
    - バリアント → `format_variant_init_line`
    - 構造体 → `format_struct_init_line`
    - それ以外の値 → `format_value_line`
    - 振り分けのあとの共通処理(ニーモニックのサイズ調整、`{src2}`の置換)は`format_line`に残している
- `format_expr_inst`(式の命令を作る)
    - テンプレートを引くキー → `expr_tmpl_key`(`Mul`は`add`のテンプレートを`imul`に置き換えて使う)
    - ニーモニックのサイズ調整に渡す実際のニーモニック → `expr_mnemonic`(`cmp_l`などのキーではなく`cmp`)
    - `/`と`%`は`format_div_expr_inst`(`div_ir.rs`)へ

- `2026-10-13`配列の添字が普通の変数(ローカル変数/関数の引数)の場合(`[arr index2 else 0] = 10`)に対応。`operand_txt.rs`の`insert_arr_txt`が`Inst::Mov`/`Inst::Param`の添字を`arr.rs`の`ref_arr_for_var`へ渡す。`ref_arr_for_struct_member`と`ref_arr_for_var`は、添字のオペランドを受け取る共通の`ref_arr_indexed`(一時レジスタの解放→基準の取得→添字の読み込み→`neg`/補正→組み立て)を使う

## 三項演算子(2026-10-18)
`cond ? left : right`のうち、条件がメモリに触れず両辺が整数だけの式のもの(`ir/builder.rs`の`ternary_is_select`)を
`cmp`+`cmov`で生成する。それ以外はIR側で条件分岐(ラベルとジャンプ)に展開されるので、ここには来ない。
生成は`asm_emitter/ternary.rs`(`gen_expr_asm`が`Ternary*`を振り分ける)。

IRの並び(`gen_ternary_select_ir` / `gen_ternary_cond_ir`)

| 順 | IR | 生成するもの |
|----|----|--------------|
| 1 | 左辺(真のときの値) | (通常の式) |
| 2 | 右辺(偽のときの値) | (通常の式) |
| 3 | `&&`/`||`の畳み込み(`AndCalcul`/`OrCalcul`) | (通常の式。各比較は0/1) |
| 4 | `Expr { kind: Ternary<比較>, ls, rs }`(番号`n-1`) | `cmp`だけ(`gen_ternary_cmp_asm`)。結果のレジスタは作らない |
| 5 | `Expr { kind: Ternary, ls: 左辺, rs: 右辺 }`(番号`n`) | `mov 右辺, dst` → `cmov<cc> 左辺, dst`(`gen_ternary_select_asm`) |

- 左辺・右辺は条件より先に生成する。`add`/`sub`は`cmp`のフラグを壊すため、`cmp`は選ぶ命令の直前に置く(`mov`はフラグを変えない)。
- `Ternary`は直前(番号`n-1`)が`Ternary<比較>`であることを前提にする(違えばpanic)。
- `&&`/`||`は、各比較を通常の比較(0/1)で作って`and`/`or`で畳み込み、最後に`TernaryNotEq(結果, 0)`を比較する。`!(..)`は`TernaryEqual(結果, 0)`。
- 符号なし整数(どちらかのオペランドが符号なし)の`<` `>` `<=` `>=`は`cmovb`/`cmova`/`cmovbe`/`cmovae`(`get_unsigned_cmov_tmpl`)。
- `cmp`の比較される側・`cmov`の元は即値にできないので、即値のときは一時レジスタへ`mov`してから使う。
- `cmov`は8bitのレジスタを使えないので、8bitの値は32bitのレジスタ名で操作する。
- `cmov`のテンプレートにはサイズ接尾辞を付けない(`fmt_mnemonic_resize`の`mov`の置き換えが`cmov`にも当たるため通さない)。
- 小数は使えない(panic)。IR側が小数を含む三項演算子を条件分岐へ回す。
- 未使用の変数の初期値が三項演算子のときは、条件(`n-1`)も一緒に生成しない(`reg_mnger.rs`の`collect_pure_expr_vars`)。
- フォーマット(`asm_fmt/x64.rs`)のキー: `tcmp`(`cmp`のみ)、`cmov_l`/`cmov_g`/`cmov_l_e`/`cmov_g_e`/`cmov_e`/`cmov_ne`、符号なし用の`cmov_b`/`cmov_a`/`cmov_b_e`/`cmov_a_e`。
  以前の`cmov_*`は`mov`+`cmov`の2行で、`cmp`が無くフラグが未設定のまま`cmov`していたため、`cmov`の1行にした。

## setcc(2026-10-19)
`ExprKind::SetLessThen` / `SetGreaterThen` / `SetLessThenEq` / `SetGreaterThenEq` / `SetEqual` / `SetNotEq`は、
比較の結果(0/1)をレジスタへ置く。生成は`asm_emitter/set_cc.rs`(`gen_expr_asm`が`Set<比較>`を振り分ける)。

```text
cmp    rs, ls          # tcmp
setcc  dst8            # set_l / set_g / set_l_e / set_g_e / set_e / set_ne
movzbl dst8, dst32     # movzbl(上位32bitも0になる)
```

- 結果のレジスタは`ls`/`rs`のレジスタとは別に確保する。`ls`が即値なら、結果のレジスタへ`mov`してから`cmp`する。
- どちらかのオペランドが符号なしの`<` `>` `<=` `>=`は`setb`/`seta`/`setbe`/`setae`(`get_unsigned_set_tmpl`。`set_b`などのキーがあればそれを使う)。
- `setcc`/`movzbl`にはサイズ接尾辞を付けない。小数は使えない(panic)。
- フォーマット(`asm_fmt/x64.rs`)のキー: `set_l`/`set_g`/`set_l_e`/`set_g_e`/`set_e`/`set_ne`、符号なし用の`set_b`/`set_a`/`set_b_e`/`set_a_e`、`movzbl`。
- `asm_fmt/x64.rs`の8bitレジスタ(`db`)の並びが、他(`dw`/`dd`/`dq`)と1つずれていた(`spl`/`bpl`が入っていて、`db[4]`が`sil`ではなく`spl`だった)。
  `setcc`で8bitレジスタ名を使うため`spl`/`bpl`を除いた。`asm_fmts/gcc_x64.yaml`に同じ並びがあれば直す。
