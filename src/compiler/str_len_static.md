# 文字列の長さの静的データ `M{n}Len`

関数の引数に `data"len"` を渡すと、長さを即値にせず、静的データ `M{n}Len` に入れて参照する。

```
M0     "hello world"
M0Len  11
```

実際のアセンブリ(GAS, AT&T)では次のように出力される。

```
M0:    .ascii "hello world"
.align 8
M0Len: .quad 11
```

呼び出し側は `movq M0Len(%rip), %rsi` のように、`M0Len` の中身を引数レジスタへ渡す。

---

## 全体の流れ

| 段階 | ファイル | やること |
| :--- | :--- | :--- |
| 1. パース | `parse/expr*.rs`(既存) | `data"len"` を `ExprNode::GetContractMemLen(Var("data"))` にする |
| 2. IR生成 | `ir/builder/proc_fn_info.rs` | 関数の引数が静的な文字列の長さなら `Inst::StrLen` を作る |
| 3. IR定義 | `ir/inst.rs` | `Inst::StrLen` の定義と型の取得 |
| 4. アセンブリ出力 | `asm_gen/call_func.rs`, `asm_emitter.rs` ほか | `M{n}Len` を `.data` に出力し、オペランドを `M{n}Len(%rip)` にする |
| (参考) 定数畳み込み | `optimization/.../constant_fold.rs`, `tracking_contract_len.rs` | `StrLen` にならなかった `data"len"` を即値にする |

---

## 1. IR: `Inst::StrLen`(`ir/inst.rs`)

```rust
StrLen {
    dst: ValueId,    // この命令自身のid
    str_id: ValueId, // 対応する `Inst::Str`(= `M{n}`)のid
    len: usize,      // `M{n}Len` に入れる値(文字列のバイト数)
    size: types::Size, // `M{n}Len` の大きさ(関数の引数の型)
}
```

- `str_id` が `M{n}` の `n` を決める元になる。`M{n}Len` の名前は、アセンブリ出力のときに `Str` のラベルに `Len` を付けて作る。
- `value_ty` / `get_param_ty` は `size` を返す。

## 2. IR生成(`ir/builder/proc_fn_info.rs`)

`gen_call_fn_ir` が引数を1つずつ処理するとき、先に `gen_str_len_arg_ir` を呼ぶ。`Some(id)` が返ればそのidを引数にし、`None` なら従来どおり `gen_expr_ir` を使う。

`gen_str_len_arg_ir` が `StrLen` を作る条件は、引数が `GetContractMemLen(対象)` で、対象が次のどちらかであること。

| 対象 | 処理 |
| :--- | :--- |
| 文字列リテラルそのもの | `Inst::Str` を新しく作り、その `dst` を `str_id` にする。`len` は文字列のバイト数 |
| 変更できない変数(`mut` ではない) | 変数の定義 `Inst::Mov { src }` から `ir_tree[src]` の `Inst::Str` を辿り、そのidを `str_id` にする(変数を定義したときの `M{n}` を使い回す)。変数の使用回数(`count_var_use`)も数える |

次の場合は `None` を返し、従来の処理になる。

- `mut` の変数(別の値に変わっている可能性がある)
- 変数の定義が文字列リテラルから作られていない(`Mov` → `Str` を辿れない)
- 対象が変数でも文字列リテラルでもない

> 前提: IRのid と `ir_tree` の添字が一致していること(`gen_expr_ir` は命令を `push` してから `id_counter` を進めている)。

## 3. アセンブリ出力(`asm_gen/`)

### `call_func.rs`

- `gen_node_asm`: `Inst::StrLen` を `gen_str_len_asm` で処理する。`unused_var_nodes` の判定対象にも含める。
- `gen_str_len_asm(dst, str_id, len, size)`:
  1. `static_label(str_id)` で `Str` のラベル(`M0`)を取得する。無ければ `asm_emit_label_not_found` を返す。
  2. `M0` + `Len` で `M0Len` の名前を作る。
  3. まだ出力していなければ、`get_static_len_fmt` で `M0Len` を `.data` へ追記する(同じ文字列は1回だけ)。
  4. `data_map` に `(dst, "M0Len")` を登録する。

### `asm_emitter.rs`

- `extract_operand_text`: `Inst::StrLen` のオペランドは `static_label(dst)` で `M0Len` を取り、`fmt_static_var_rip` で `M0Len(%rip)` にする。
  (この分岐が無いと、最後の `expr_result_operand` に落ちて panic する)

### `asm_fmt/`

- `x64.rs`: テンプレート `data_quad`(`.align 8\n{name}: .quad {value}\n`)を追加。
- `mng_fmt/emit_value_asm.rs`: `get_static_len_fmt(len, label, size)`。`size` が8バイト以上なら `data_quad`、それ以外は `data_long`(`.long`)。

### `lazy_spill.rs`

- `StrLen` は変数を参照しないノードとして扱う(`Num` / `Float` / `Str` と同じ)。

## 4. 即値になる場合(`optimization/`)

`gen_str_len_arg_ir` が `None` を返した `data"len"` は、これまでどおり `gen_expr_ir` の `GetContractMemLen` で `fold_expr` により即値へ畳み込まれる。

- `contract_mem_len`(`tracking_contract_len.rs`): `Str` / `Array` のリテラルは長さ、`Var` は `VarTree` から引いた長さを返す。分からなければ `None`。
- `constant_fold.rs`: `None` は `0` として扱う。

---

## 動きの例

```
data: [b1 11] = "hello world"
write(data, data"len")
```

| id | IR | 備考 |
| :--- | :--- | :--- |
| 0 | `Str { dst: 0, value: "hello world" }` | `M0` |
| 1 | `Mov { name: "data", src: 0 }` | 変数の定義 |
| 2 | `StrLen { dst: 2, str_id: 0, len: 11, size: DQ }` | `M0Len` |
| 3 | `CallFunc write(params: [..., 2])` | |

アセンブリでは、`M0` と `M0Len` を `.data` に出力し、2つ目の引数に `M0Len(%rip)` の値を渡す。

## 制限

- 文字列は `.ascii` で出力するので、末尾に NUL は付かない(既存の挙動)。
- 長さは文字列のバイト数(`value.len()`)。
- `mut` の変数や、長さが静的に分からないものは `M{n}Len` にならない。
- `str_id` に対応する `Str` が出力されていない場合(`unused_var_nodes` で省かれたなど)は、`asm_emit_label_not_found` になる。