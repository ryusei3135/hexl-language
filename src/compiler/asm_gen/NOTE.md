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
