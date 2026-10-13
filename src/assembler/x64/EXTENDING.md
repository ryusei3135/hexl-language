# 機能の追加方法

やりたいことごとに、編集するファイルは決まっています。

| やりたいこと | 編集するファイル | 内容 |
|---|---|---|
| レジスタを追加 | `reg.rs` | `define_regs!` に1行足す |
| 命令を追加 | `emitter/inst_table.rs` | `dispatch` の `match` に1行 + 同じファイル下部に `emit_xxx` を書く |
| SSE 命令 (`xmm/m -> xmm` 形式) を追加 | `emitter/inst_table.rs` | `sse_spec` の `match` に1行 (プレフィックスとオペコード) |
| ディレクティブを追加 (既存の出力処理で表現できる場合) | `convert/directives.rs` | `convert_directive` の `match` に1行 |
| ディレクティブを追加 (新しい出力動作が必要な場合) | `convert/directives.rs` + `emitter.rs` | 上に加えて `enum Directive` と `emit_directive` |
| 出力形式を追加 | `format_table.rs` (+ 新しい形式の `.rs`) | `FORMATS` に1行 |

各ファイルの先頭のコメントに、具体的な手順と見本があります。
命令の見本は `emitter/inst_table.rs` の `nop` です。

即値とメモリオペランドの組み合わせは、メモリ幅をレジスタから推測できません。
`cmpb` / `cmpw` / `cmpl` / `cmpq` のようにニーモニックのサフィックスを
`emitter/inst_table.rs` でビット幅に変換して `emitter.rs` へ渡し、
`80 /digit` (byte) または `81` / `83 /digit` の即値グループ形式で出力します。

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

## SSE (浮動小数点) 命令

`movss` / `movsd` / `movups` / `movaps`、`add` / `sub` / `mul` / `div` / `sqrt` の `ss` / `sd` / `ps`、
`comiss` / `ucomiss` / `comisd` / `ucomisd`、`cmpps` (と `cmpeqps` などの述語付き別名) に対応しています。
AT&T 構文で `op 元, 先` の順に書きます (`cmpps` だけ先頭に `$imm`)。

- 使えるレジスタは `%xmm0`-`%xmm15`。`%ymm` / `%zmm` は `reg.rs` に定義済みですが、
  VEX / EVEX 符号化が必要なため、SSE 命令に渡すとエラーになります。
- SSE 以外の命令 (`add` など) にベクトルレジスタを渡した場合もエラーになります。
- `cmpps` でメモリが `symbol(%rip)` の場合は未対応です (末尾の imm8 の分だけ RIP 相対の再配置がずれるため)。
  `cmpl $1, symbol(%rip)` のような即値付き命令にも同じ制約があります。
- 符号化は GNU as の出力と一致することをテスト (`sse_tests`) で確認しています。
