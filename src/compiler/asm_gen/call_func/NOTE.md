
## 修正項目
### 関数
1. `assign_val_ty_is_ptr`
    - サイズが常に8byte

## リファクタリング(2026-10-11)
長かった関数を、役割ごとの小さい関数に分けた。**生成されるアセンブリは変えていない**
(処理の順序・使うレジスタ・スタックの確保の順番はそのまま)。

### `call_func.rs`
#### `build_fn_process`(関数1つぶんのアセンブリを作る)
| 順 | 処理 | 関数 |
|----|------|------|
| 1 | ラベル名を作る(ジェネリクスは型引数を含める) | `fn_label_of` |
| 2 | 関数ごとの状態の初期化、変数へのレジスタ割り当て | `begin_fn_state` |
| 3 | 仮のスタックフレームを出力し、位置(`prologue_start`/`prologue_end`)を控える | `gen_fn_prologue` |
| 4 | 戻り値の構造体を探す | `find_returned_struct_idx` |
| 5 | IRのノードを順に変換する | `gen_node_asm` |
| 6 | `ret`が無ければ`leave; ret`を付ける(`_start`は付けない) | `gen_implicit_ret` |
| 7 | `callee_saved`の`push`/`pop`を、目印と置き換えて確定する | `fix_callee_saved` |
| 8 | 数えたスタック使用量で、スタックフレームを作り直す | `gen_fn_prologue` |

- 関数の生成中、どのノードでも共通して使う情報は`FnGenCtx`にまとめて`gen_node_asm`へ渡す。
    - `fn_name` / `fn_ret_ty` / `this_is_self` / `returned_struct_idx` / `asm_fmt_name`
- `gen_node_asm`は、ノードの種類ごとに処理先を振り分けるだけ。長くなる処理は別関数にしている。
    - `Inst::Mov` → `gen_mov_node_asm`(戻り値の構造体なら`mov_value_ir`を呼ばない判定)
    - `Inst::Param` → `gen_param_asm`(引数を変数として登録)
    - `Inst::CallFunc` → `gen_call_node_asm`(戻り値を代入しない呼び出しのみ生成)
- 手順7で`{callee_save}`/`{callee_restore}`の目印を置き換えるのは、使う`callee_saved`の
    レジスタが、本体を生成し終わるまで分からないため。**手順8より先に**本体側を置き換える
    (手順8は`prologue_end`より前だけを書き換える)。

#### `emit_call_func`(関数呼び出しのアセンブリを作る)
| 順 | 処理 | 関数 |
|----|------|------|
| 1 | 使用中の`caller_saved`レジスタを`push`で退避 | `push_caller_saved_regs` |
| 2 | 引数ごとに`(引数レジスタの番号, 読み出し元, asm)`を作る | `build_arg_lines` |
| 3 | 引数レジスタを壊さない順序に並べる | `order_arg_lines` |
| 4 | `call`を出力 | `get_call_func_fmt` |
| 5 | 退避したレジスタを逆順に`pop` | `pop_caller_saved_regs` |

- `build_arg_lines`の中で、値を渡す命令を`arg_opcode`が決める。
    - `"address"`(`lea`): 構造体を返す呼び出しの先頭引数 / ポインタ / 文字列
    - `"mov"`: それ以外
- `order_arg_lines`: `f(y, x)`で`x`が`%rdi`にある場合など、先に`%rdi`へ書くと
    後ろの引数が壊れた値を読む。他の未出力の引数から読まれていない引数レジスタを持つものから出力する。
    循環している場合のみ元の順で出力する。

### `call_func/mem_ir.rs`
#### `mov_value_ir`(変数の定義 `Inst::Mov`)
変数の型と初期値で、次のどれか1つに振り分ける(上から順に判定)。

| 条件 | 関数 | 内容 |
|------|------|------|
| ポインタ変数 ← 構造体リテラル(`p: Name* = Name {..}`) | `mov_struct_literal_to_ptr` | 実体はスタック、変数は先頭アドレスのレジスタ |
| 構造体の変数 | `mov_struct_var` | `VarIndexInfo::new_stack`で`%rbp`からのオフセットを登録 |
| 静的領域の値 ← ポインタでない変数 | `mov_static_var` | 変数名だけ登録 |
| それ以外 | `mov_reg_var` | レジスタに置く変数 |

- 共通の補助関数
    - `struct_literal_ptr_target`: 1番目の条件の判定と、指す先の構造体の型の取得
    - `bind_var_reg`: 変数のレジスタを決めて使用中にする(`reg_mnger.rs`が割り当てた空きレジスタを優先、無ければ`alloc_reg`)
    - `reserve_ctor_self_stk`: コンストラクタで初期化する構造体の領域を、先に`stk_use_counter`から確保(配列との重なり防止)
    - `ctor_self_stk_offset`: コンストラクタ呼び出しの暗黙の`self`が指す領域の`%rbp`からのオフセット
    - `init_reg_line`: レジスタへ初期値を書く命令(ポインタ型への数値リテラルは`movq`、ポインタは`lea`、それ以外は`mov`)

#### `mem_val_ir`(`Inst::MemoryValue`)
- 静的領域(`MemoryKind::Static`) → `is_static_var`
- それ以外(配列) → `stack_arr_ir`
    - 領域全体(`要素のサイズ * 要素数`)を先に確保し、要素`k`は`arr_base + (k+1) * 要素のサイズ`に置く
