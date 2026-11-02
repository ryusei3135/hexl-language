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

### スカラー浮動小数点と整数の変換

`cvtsi2ss` / `cvtsi2sd` は32/64ビット整数レジスタから `%xmm` へ、
`cvttss2si` / `cvttsd2si` は `%xmm` またはメモリから32/64ビット整数レジスタへ切り捨て変換します。
整数レジスタの幅はレジスタ名から判定します。整数メモリソースはサフィックスなし / `l` で32ビット、
`q` で64ビットとして扱います (`cvtsi2ssq` / `cvtsi2sdq`)。
整数幅サフィックスは `cvttss2sil` / `cvttss2siq` にも指定でき、出力レジスタ幅と一致する必要があります。

`cvtss2sd` と `cvtsd2ss` は `%xmm` またはメモリから `%xmm` への変換です。
これらは64ビット整数幅の指定を受け付けません。

## 小数データ (`.float` / `.double`)

`.float` (別名 `.single`、4バイト IEEE 754 単精度) と `.double` (8バイト 倍精度) に対応しています。

```gas
.data
.align 4
M0: .float 1.1
.align 4
M1: .float 2.1
```

- 引数は `1.1` / `-0.5` / `2.5e-2` のような小数、または `3` のような整数 (小数として出力)。カンマ区切りで複数指定できます。
- レキサは `.` の直後に数字が続くときだけ小数リテラル (`Token::Float`) とみなします。
- 小数リテラルはディレクティブの引数専用です (`movss $1.5, %xmm0` のように命令オペランドに書くとエラー)。
- `.text` セクションでは使えません (`.data` / `.section test` のみ)。
- 先頭が `.` の小数 (`.5`) は `.2byte` などと区別できないため未対応です。`0.5` と書いてください。
- 符号化は GNU as の出力と一致することをテスト (`float_tests`) で確認しています。

## 論理演算とシフト (`and` / `or` / `xor` / `shl` / `sal` / `shr` / `sar`)

`emitter/inst_table.rs` の `emit_alu` (`and` / `or` / `xor`) と `emit_shift` (シフト) が担当します。
AT&T 構文で `op 元, 先` の順に書きます。サフィックス (`b` / `w` / `l` / `q`) は省略できます。

| 形 | 例 |
|---|---|
| 即値 → レジスタ | `and $0xff, %eax` / `orq $1, %rbx` |
| 即値 → メモリ | `andl $1, -4(%rbp)` / `orb $0x80, -1(%rbp)` |
| レジスタ → レジスタ/メモリ | `xor %edx, %edx` / `and %eax, -4(%rbp)` |
| メモリ → レジスタ | `or -4(%rbp), %ecx` |
| シフト (即値) | `shl $3, %eax` / `sarq $63, -8(%rbp)` |
| シフト (`%cl`) | `shr %cl, %rdx` / `shll %cl, -4(%rbp)` |
| シフト (1bit) | `shl %eax` / `shl $1, %eax` |

- サイズはレジスタ名で決まります。メモリ + 即値、メモリのシフトはサフィックスで決まり、無印は32bitです。
  サフィックスとレジスタ幅が食い違う場合はエラーです。
- シフト量のレジスタは `%cl` のみ、即値は 0〜255 です。`$1` は GNU as と同じく `D0` / `D1` の短縮形になります。
- `%al` / `%eax` などへの即値は、GNU as と同じ短縮形 (`24 ib` / `25 id` など) を選びます。
- `symbol(%rip)` + 即値 (`andl $1, foo(%rip)` / `shll $2, foo(%rip)`) は、`cmpl` と同じ理由で未対応です。
- 符号化は GNU as の出力と一致することをテスト (`alu_shift_tests`) で確認しています。

## インクリメントとデクリメント (`inc` / `dec`)

`emitter/inst_table.rs` の `emit_inc_dec` が担当し、レジスタとメモリの両方を処理します。
サイズサフィックス (`b` / `w` / `l` / `q`) はメモリオペランドで必要です。レジスタのサイズはレジスタ名から決まり、サフィックスが不一致ならエラーです。
符号化は GNU as の出力と一致するテスト (`inc_dec_tests`) で確認します。

## 数値リテラル

10進数に加えて、16進数 (`0xff` / `0XFF`) を書けます。`-0x10` のように前に `-` を付けられます。

## 条件付き命令 (`jcc` / `cmovcc` / `setcc`)

条件 (`o` / `b` / `e` / `l` / `g` ... と別名 `z` / `nz` / `nae` など) は `emitter/inst_table.rs` の
`JCC_TABLE` に一括で定義されており、`jcc` / `cmovcc` / `setcc` が共有します。

- `setcc` (`sete` / `setne` / `setl` / `setg` / `setb` / `seta` ...) は `emit_setcc` が担当します。
  オペランドは8bitのレジスタ (`%al` ... `%r15b`) またはメモリ (サフィックスなし) です。
  `%spl` / `%bpl` / `%sil` / `%dil` には空の REX (`40`) を付けます (`needs_empty_rex`)。
  8bit以外のレジスタ、即値、オペランド数の違いはエラーです。
- `cmovcc` は16/32/64bitのレジスタ宛てで、`cmovl` / `cmovnel` のようにサフィックスを付けられます。
- 符号化は GNU as の出力と一致することをテスト (`jcc_tests` / `cmov_tests` / `setcc_tests`) で確認しています。

## lea のレジスタ同士

`leaq %rax, %rcx` のように元・先がともにレジスタの場合は、`movq %rax, %rcx` と同じ符号化で出力します
(実機の `lea` はメモリオペランド必須で、`mod=11` は #UD になるため)。
サイズ不一致 (`leaq %eax, %rcx` など) は `register size mismatch` エラーです。

## 文字列命令と `rep` 系プレフィックス (`movs` / `cmps` / `stos` / `lods` / `scas`)

`emitter/inst_table.rs` の `string_op_spec` (表) と `emit_string_op` / `emit_rep` が担当します。
オペランドは暗黙 (`%rsi` / `%rdi` / `%al` など) なので、書くのはニーモニックだけです。

| 命令 | 符号化 (8bit 版 / 16・32・64bit 版) | 動作 |
|---|---|---|
| `movsb` `movsw` `movsl` `movsq` | `A4` / `A5` | `[%rdi] <- [%rsi]` |
| `cmpsb` `cmpsw` `cmpsl` `cmpsq` | `A6` / `A7` | `[%rsi]` と `[%rdi]` を比較 |
| `stosb` `stosw` `stosl` `stosq` | `AA` / `AB` | `[%rdi] <- %al/%ax/%eax/%rax` |
| `lodsb` `lodsw` `lodsl` `lodsq` | `AC` / `AD` | `%al/%ax/%eax/%rax <- [%rsi]` |
| `scasb` `scasw` `scasl` `scasq` | `AE` / `AF` | `%al/%ax/%eax/%rax` と `[%rdi]` を比較 |
| `cld` / `std` | `FC` / `FD` | 方向フラグ DF のクリア / セット |

- サフィックスは必須です (`b` / `w` / `l` / `q`)。サイズを書かない `movs` などはエラーです。
- `movsd` / `cmpsd` は SSE 命令と同じ名前です。**オペランドが無いときだけ**32bit の文字列命令
  (`movsl` / `cmpsl` と同じ) になり、オペランドが有るときは従来どおり SSE 命令です。
  GNU as に `stosd` / `lodsd` / `scasd` は無いので、これらもエラーです。
- `rep` / `repe` / `repz` は `F3`、`repne` / `repnz` は `F2` を付けます。GNU as と同じく、
  どのプレフィックスも5種類の文字列命令すべてに付けられます (`rep scasb` は `repe scasb` と同じ `F3 AE`)。
- 出力の並びは `[66] [F3/F2] [REX.W] opcode` です (例: `rep movsw` は `66 F3 A5`、`rep movsq` は `F3 48 A5`)。
- パーサは `rep movsb` を「ニーモニック `rep` + シンボル `movsb`」と読みます。
  `rep` の後に文字列命令が同じ行に無い場合 (`rep` だけ、`rep nop`、`rep ret` など) はエラーです。
- 明示オペランド付きの形 (`stosb %al, (%rdi)` など) は未対応です。

```gas
cld                  # 前方向 (DF=0)
movq $16, %rcx
rep movsb            # %rcx バイトを [%rsi] から [%rdi] へコピー

movq $0, %rax
repne scasb          # %al と一致するバイトまで [%rdi] を進める (見つかれば ZF=1)
```

## `test`

`emit_test` が担当します。`and` と同じく結果は捨て、フラグだけを更新します。
サフィックス (`b` / `w` / `l` / `q`) は省略できます。

| 形 | 例 |
|---|---|
| レジスタ同士 | `testb %dl, %dl` / `test %eax, %ebx` |
| レジスタとメモリ | `testl %eax, -4(%rbp)` / `testl 8(%r9), %eax` |
| 即値とレジスタ | `testb $1, %al` / `testq $0x1000, %rcx` |
| 即値とメモリ | `testl $1, -4(%rbp)` / `testb $0x80, (%rax)` |

- `test` には 8bit 即値で済ませる短縮形 (`83`) が無いので、即値は常にオペランドサイズ幅で出力します
  (`testl $1, %ecx` は `F7 C1 01 00 00 00`)。`%al` / `%ax` / `%eax` / `%rax` だけは `A8` / `A9` の短縮形です。
- メモリ + 即値のサイズはサフィックスで決まり、無印は32bitです。
- `symbol(%rip)` + 即値は、`cmpl` などと同じ理由で未対応です。
- 符号化は GNU as の出力と一致することをテスト (`string_rep_tests`) で確認しています。
  `file.hexl` のインラインasm (`os_copy` / `os_find`) と同じ並びのテストも含みます。

## 既知の差 (GNU as との違い)

`movq $-1, %rax` のように 64bit レジスタへ符号拡張で収まる即値を入れる場合、GNU as は
`48 c7 c0 ff ff ff ff` (7バイト) を出しますが、このアセンブラは `movabs` (`48 b8` + 8バイト) を出します。
どちらも同じ意味で、動作には影響しません。

## 文字列命令と `rep` 系プレフィックス (`movs` / `cmps` / `stos` / `lods` / `scas`)

`emitter/inst_table.rs` の `string_op_spec` (表) と `emit_string_op` / `emit_rep` が担当します。
オペランドは暗黙 (`%rsi` / `%rdi` / `%al` など) なので、書くのはニーモニックだけです。

| 命令 | 符号化 (8bit 版 / 16・32・64bit 版) | 動作 |
|---|---|---|
| `movsb` `movsw` `movsl` `movsq` | `A4` / `A5` | `[%rdi] <- [%rsi]` |
| `cmpsb` `cmpsw` `cmpsl` `cmpsq` | `A6` / `A7` | `[%rsi]` と `[%rdi]` を比較 |
| `stosb` `stosw` `stosl` `stosq` | `AA` / `AB` | `[%rdi] <- %al/%ax/%eax/%rax` |
| `lodsb` `lodsw` `lodsl` `lodsq` | `AC` / `AD` | `%al/%ax/%eax/%rax <- [%rsi]` |
| `scasb` `scasw` `scasl` `scasq` | `AE` / `AF` | `%al/%ax/%eax/%rax` と `[%rdi]` を比較 |
| `cld` / `std` | `FC` / `FD` | 方向フラグ DF のクリア / セット |

- サフィックスは必須です (`b` / `w` / `l` / `q`)。サイズを書かない `movs` などはエラーです。
- `movsd` / `cmpsd` は SSE 命令と同じ名前です。**オペランドが無いときだけ**32bit の文字列命令
  (`movsl` / `cmpsl` と同じ) になり、オペランドが有るときは従来どおり SSE 命令です。
  GNU as に `stosd` / `lodsd` / `scasd` は無いので、これらもエラーです。
- `rep` / `repe` / `repz` は `F3`、`repne` / `repnz` は `F2` を付けます。GNU as と同じく、
  どのプレフィックスも5種類の文字列命令すべてに付けられます (`rep scasb` は `repe scasb` と同じ `F3 AE`)。
- 出力の並びは `[66] [F3/F2] [REX.W] opcode` です (例: `rep movsw` は `66 F3 A5`、`rep movsq` は `F3 48 A5`)。
- パーサは `rep movsb` を「ニーモニック `rep` + シンボル `movsb`」と読みます。
  `rep` の後に文字列命令が同じ行に無い場合 (`rep` だけ、`rep nop`、`rep ret` など) はエラーです。
- 明示オペランド付きの形 (`stosb %al, (%rdi)` など) は未対応です。

```gas
cld                  # 前方向 (DF=0)
movq $16, %rcx
rep movsb            # %rcx バイトを [%rsi] から [%rdi] へコピー

movq $0, %rax
repne scasb          # %al と一致するバイトまで [%rdi] を進める (見つかれば ZF=1)
```

## `test`

`emit_test` が担当します。`and` と同じく結果は捨て、フラグだけを更新します。
サフィックス (`b` / `w` / `l` / `q`) は省略できます。

| 形 | 例 |
|---|---|
| レジスタ同士 | `testb %dl, %dl` / `test %eax, %ebx` |
| レジスタとメモリ | `testl %eax, -4(%rbp)` / `testl 8(%r9), %eax` |
| 即値とレジスタ | `testb $1, %al` / `testq $0x1000, %rcx` |
| 即値とメモリ | `testl $1, -4(%rbp)` / `testb $0x80, (%rax)` |

- `test` には 8bit 即値で済ませる短縮形 (`83`) が無いので、即値は常にオペランドサイズ幅で出力します
  (`testl $1, %ecx` は `F7 C1 01 00 00 00`)。`%al` / `%ax` / `%eax` / `%rax` だけは `A8` / `A9` の短縮形です。
- メモリ + 即値のサイズはサフィックスで決まり、無印は32bitです。
- `symbol(%rip)` + 即値は、`cmpl` などと同じ理由で未対応です。
- 符号化は GNU as の出力と一致することをテスト (`string_rep_tests`) で確認しています。
  `file.hexl` のインラインasm (`os_copy` / `os_find`) と同じ並びのテストも含みます。

## 既知の差 (GNU as との違い)

`movq $-1, %rax` のように 64bit レジスタへ符号拡張で収まる即値を入れる場合、GNU as は
`48 c7 c0 ff ff ff ff` (7バイト) を出しますが、このアセンブラは `movabs` (`48 b8` + 8バイト) を出します。
どちらも同じ意味で、動作には影響しません。
