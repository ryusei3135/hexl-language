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
