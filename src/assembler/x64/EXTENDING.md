# 機能の追加方法

やりたいことごとに、編集するファイルは決まっています。

| やりたいこと | 編集するファイル | 内容 |
|---|---|---|
| レジスタを追加 | `reg.rs` | `define_regs!` に1行足す |
| 命令を追加 | `emitter/inst_table.rs` | `dispatch` の `match` に1行 + 同じファイル下部に `emit_xxx` を書く |
| ディレクティブを追加 (既存の出力処理で表現できる場合) | `convert/directives.rs` | `convert_directive` の `match` に1行 |
| ディレクティブを追加 (新しい出力動作が必要な場合) | `convert/directives.rs` + `emitter.rs` | 上に加えて `enum Directive` と `emit_directive` |
| 出力形式を追加 | `format_table.rs` (+ 新しい形式の `.rs`) | `FORMATS` に1行 |

各ファイルの先頭のコメントに、具体的な手順と見本があります。
命令の見本は `emitter/inst_table.rs` の `nop` です。

## `test` セクション (`.section test`)

`.section test` (または `.test`) で、名前 `test` の独立したデータセクションに切り替わります
(`.byte` / `.quad` / `.ascii` などのデータ定義やラベルが使えます)。

| 出力 | `test` を使ったときの結果 |
|---|---|
| `-o` (ELF オブジェクト) | セクションヘッダ `test` (PROGBITS, WA) を追加。必要なら `.rela.test` も出力 |
| `-c linux` (ELF 実行ファイル) | セクションヘッダテーブル (.text / .data / .shstrtab) を常に出力。`test` 使用時は `test` も追加 |
| `-c win` (PE32+) | セクションヘッダ `test` (初期化済みデータ, RW) を追加し、次のページに配置 |

`-o` と `-c win` は `test` を使わなければ従来と同一です (`-c linux` はヘッダ追加のため末尾が増えます)。
他のセクション名を足したい場合は `emitter.rs` の `enum Section` と `convert/directives.rs`
の `.section` の `match` に足し、`elf.rs` / `pe.rs` に同様のヘッダ出力を追加してください。
