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
