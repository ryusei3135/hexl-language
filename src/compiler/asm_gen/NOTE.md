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
- `AsmEmitter`に状態のフィールドを追加したら、`emit_callee_fn`の退避・復元にも加えること。

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
- `AsmEmitter::unused_var_nodes`は関数ごとの状態なので、`emit_callee_fn`で退避・復元する。
