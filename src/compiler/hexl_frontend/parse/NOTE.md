`[func.rs]`にdefine_arg_nodeで、引数が不変かを判別する場所がある
`[stmt.rs]`でノードを管理する

## panic/unwrapをErrで返す修正(2026-10-15)
- テスト以外の`panic!`/`unwrap`/`expect`/`unreachable!`を、`Err(err::ErrKind)`で返すようにした。
- 汎用のエラー生成関数は`err_factory/common_err.rs`
    - `expected_tkn(expected, found)` / `expected_at_current(expected)`: 期待したトークンではなかった
    - `invalid_number(num)`: 数値として読み取れない
    - `self_outside_of_type()`: 構造体/バリアントの外で`Self`を使った
    - `internal_state_err(what)`: パーサーの内部状態が想定外(本来は起こらない)
- `tkn_mnger.rs`の`advance_tkn_checked`: `advance_tkn().unwrap()`の代わり(次のトークンが無ければEOFのエラー)
- シグネチャを変えた関数(呼び出し側で`?`が必要)
    - `Group2Node::change_group1` -> `Result<Group1Node, ErrKind>`
    - `ModPath::add_path` -> `Result<(), ErrKind>`
    - `ModPath::gen_parent_path` -> `Result<String, ErrKind>`(`Literal`形式では`Err`)
    - `registration_mod_name`(`typedef/def_fields.rs`) -> `Result<(), ErrKind>`
- `ModPath::last_segment`は、パスが空のとき空文字列を返す(panicしない)
- 例外: `tkn_mnger.rs`の`current_tkn`は`&Tkn`を返す関数で、`tkns`が未設定/範囲外のときの
    値が無いため、`unwrap`が残っている(`current_line`/`tkn_chr_pos`はpanicしない)

## 2026-10-25
- 型のバイトサイズを取得する機能`sizeof`を追加 expr.rsのvalueに