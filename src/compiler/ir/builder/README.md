# [builder] モジュール

このモジュールは、**`ir/builder.rs`の処理の一部** を担当します。

## ファイル構成と役割

このモジュールに含まれるファイルの一覧と、それぞれの主な役割です。

| ファイル名 | 役割 / 主な機能 | 備考 |
| :--- | :--- | :--- |
| `[expr_node].rs` | [gen_expr_irの処理の一部] | - |
| `[member].rs` | [構造体のメンバーからirなどを生成する際に必要なAPI] | - |
| `[proc_fn_info].rs` | [関数のノードからirや関数の登録などをするAPI] | - |
| `[scope].rs` | [例`mod::func()`などのスコープを処理するAPI] | - |
| `[variant_bind].rs` | [`cond M(val) := value`(`Expr::VariantBind`)を`value.label == tag`と`val = value.value`に展開するAPI] | `gen_match_expr_ir`から使う |

---

## 各ファイルの依存関係・データフロー

各ファイルがどのように連携しているかの補足説明です。
* **`[expr_node.rs]`** が`IR::gen_expr_ir`の処理の一部を記述してある

## 開発者向けの注意点・メモ
* このモジュールは、`ir/builder.rs`のみ使用可能

`[scope.rs]`でスコープを管理するAPIを作成`v0.11.1`


## 変数の使用回数のカウント(`counter_mnger.rs`)
- 関数のスコープごとに`VarCounter`を1つ持ち、`FnDefInfo.var_counter`に保存する。
  関数のIRを作り始めるとき(`proc_fn_info.rs`の`ini_def_fn_info`)に、引数の数で初期化し直す。
- 普通の変数は名前ごと(`list`)、引数は左から何番目かごと(`args`)に、変数が参照されるたびに1回数える。定義は数えない。
- IRの各所からは`IR::count_var_use(名前)`を呼ぶ。引数かローカル変数かはその中で判定する。
    - `Expr::Var`(値の読み出し・代入先・`GetAddress`/`[ptr]`の対象・メゾット呼び出しの暗黙の第一引数)
    - `Expr::RefArray`(配列の添字アクセス)
    - `member_is_var` / `member_is_arr_ref` / `member_is_var_via_ptr`(構造体のメンバーアクセス)
    - `member_is_fn` / `member_is_fn_via_ptr`は、暗黙の第一引数が`Expr::Var`として処理されるので数える必要がない(二重に数えない)
- 外部の関数(`gen_fn_def`)の`FnDefInfo`は空のカウンターを持つ。処理中の関数のカウンターを渡さない。
- 使用回数が多い順は`vars_by_priority` / `args_by_priority`で取得する。

## 型が決まっていない即値の型(2026-10-16)
- `a < 10`・`10 < a`・`a + 1`のように、片方が即値(`10`・`1.5`・`1 + 2`)でもう片方が変数などの二項演算では、
  即値の型をもう片方の型(変数を定義した型)にする(`expr_node.rs`の`operand_expect_tys` / `infer_expr_ty`)。
  以前は呼び出し側の期待する型(無ければ`DD`)になり、`uint64`の変数との比較で即値が`DD`になっていた。
- 整数の即値は整数型、小数の即値は浮動小数点型のときだけ変換する。ポインタ・構造体の型は即値の型にしない。
- 期待する型が無い二項演算(`if a < 10`など)は、`resolve_expect_byte`がオペランドの型で決める(決まらなければ`DD`)。
- 関数の引数は従来どおり、関数の定義の引数の型(`gen_call_fn_ir`)。
