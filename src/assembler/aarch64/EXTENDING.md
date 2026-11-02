# aarch64 アセンブラ

`x64/` を見本にした、簡単な aarch64 (ARMv8-A, A64) アセンブラです。
文字列を渡すとレキサ → パーサ → 符号化 → ELF 出力までを行います。

```rust
use crate::assembler::aarch64::emitter_aarch64;

// 再配置可能オブジェクトファイル (a.o)
emitter_aarch64("-o", source, "linux")?;
// 単独で実行できる ELF (a.elf)
emitter_aarch64("-c", source, "linux")?;
```

出力先を指定するときは `emitter_aarch64_to(mode, source, os, Some("path"))`、
ファイルを書かず機械語だけ欲しいときは `assemble_aarch64(source)` (`(text, data)` を返す) を使います。

## 構文

gas (GNU as) の aarch64 構文を元に、x64 版と見た目を揃えて次の2点だけ変えています。

| 項目 | gas | このアセンブラ |
|---|---|---|
| レジスタ | `x0` / `w1` / `sp` | `%x0` / `%w1` / `%sp` |
| 即値 | `#8` | `$8` |

```gas
.globl _start
.text
_start:
    stp %x29, %x30, [%sp, $-16]!     // プリインデックス
    mov %x0, $40
    add %x0, %x0, $2
    adrp %x1, msg                    // adrp + :lo12: でシンボルのアドレス
    add %x1, %x1, :lo12:msg
    ldr %w2, [%x1, %x3, lsl $2]      // レジスタオフセット
    cmp %x0, $42
    b.ne fail
    ldp %x29, %x30, [%sp], $16       // ポストインデックス
    ret
```

- レジスタ: `%x0`-`%x30` / `%w0`-`%w30` / `%sp` / `%wsp` / `%xzr` / `%wzr`、別名 `%fp` (= `%x29`) / `%lr` (= `%x30`)
- メモリ: `[%x0]` / `[%x0, $8]` / `[%x0, $8]!` / `[%x0], $8` / `[%x0, %x1{, lsl $n}]` /
  `[%x0, %w1, sxtw{ $n}]` / `[%x0, :lo12:sym]`
- コメント: `//` / `/* */` / `#` (行末まで)。`;` は文の区切り
- 条件分岐は `b.eq` 形式 (`beq` は gas の aarch64 にも無いので未対応)

## 対応している命令

| 分類 | 命令 |
|---|---|
| 分岐 | `b` `bl` `br` `blr` `ret` `b.cond` `cbz` `cbnz` `tbz` `tbnz` |
| 移動 | `mov` (レジスタ / 即値) `movz` `movn` `movk` |
| 算術 | `add` `adds` `sub` `subs` `cmp` `cmn` `neg` `negs` `mul` `mneg` `madd` `msub` `sdiv` `udiv` `smulh` `umulh` |
| 論理 | `and` `orr` `eor` `ands` `bic` `orn` `eon` `bics` `tst` `mvn` |
| シフト/拡張 | `lsl` `lsr` `asr` `ror` (即値 / レジスタ) `sxtb` `sxth` `sxtw` `uxtb` `uxth` |
| 条件選択 | `csel` `csinc` `csinv` `csneg` `cset` `csetm` |
| アドレス | `adr` `adrp` |
| ロード/ストア | `ldr` `str` `ldrb` `strb` `ldrh` `strh` `ldrsb` `ldrsh` `ldrsw` `ldur`系 `stur`系 `ldp` `stp` |
| その他 | `nop` `yield` `wfe` `wfi` `sev` `isb` `svc` `brk` |

ディレクティブ: `.text` `.data` `.rodata` `.section` `.global` `.globl` `.extern`
`.byte` `.hword` `.short` `.2byte` `.word` `.long` `.int` `.4byte` `.quad` `.xword` `.8byte`
`.float` `.double` `.ascii` `.asciz` `.string` `.align` `.p2align` `.balign` `.zero` `.skip` `.space`

## gas との違い

- `mov %x0, $0x12345678` のように1命令で表せない値は、gas ではエラーですが、
  このアセンブラは `movz` + `movk` の列に展開します。
  (1命令で表せる値は gas と同じ `movz` → `movn` → `orr` の優先順で選びます)
- `.align N` / `.p2align N` は **2^N バイト** (gas の aarch64 と同じ。x64 版の `.align` はバイト数)。
  バイト数で指定するときは `.balign N` を使います。
- `.text` の `.align` は `nop` で埋めます。
- `.text` にもデータ (`.word` など) を置けますが、その後の命令が4バイト境界にないとエラーです。
- `.extern` 宣言の無い未定義シンボルは、gas と違いエラーにします (x64 版と同じ)。
- 同じセクション内の PC 相対の参照 (`b` / `bl` / `b.cond` / `cbz` / `tbz` / `ldr` リテラル / `adr`) は
  アセンブラが解決します。それ以外 (外部シンボル、別セクション、`adrp` / `:lo12:`、`.word sym` / `.quad sym`)
  は `-o` では再配置エントリ (addend は 0) になり、`-c` ではアドレスが決まった後に埋め込みます。

## 未対応

SIMD / 浮動小数点命令とレジスタ (`%v0` `%d0` `%s0` ...)、`ldrsw` のリテラル形式、`ldpsw`、
`ubfx` / `sbfx` / `bfi` などのビットフィールド命令、アトミック命令 (`ldxr` / `stxr` / `ldar` ...)、
`mrs` / `msr`、バリア (`dmb` / `dsb`)、`.type` / `.size` / `.cfi_*`、`.bss`、PE (Windows) 出力。

# 機能の追加方法

やりたいことごとに、編集するファイルは決まっています。

| やりたいこと | 編集するファイル | 内容 |
|---|---|---|
| レジスタを追加 | `reg.rs` | `define_regs!` に1行足す |
| 命令を追加 | `emitter/inst_table.rs` | `dispatch` の `match` に1行 + 同じファイル下に `emit_xxx` を書く |
| 固定1語の命令 (`nop` など) を追加 | `emitter/inst_table.rs` | `fixed_word` の `match` に1行 |
| 条件コードを追加 | `emitter/inst_table.rs` | `CONDITIONS` に1行 |
| ディレクティブを追加 | `directives.rs` | `emit_directive` の `match` に1行 |
| 再配置の種類を追加 | `emitter.rs` | `FixupKind` に variant を足し、`elf_reloc_type` / `patch` (必要なら `is_pc_relative`) に1行ずつ |
| 新しい構文 (オペランドの形) を追加 | `parse.rs` | `Operand` / `parse_operand` |
| 出力形式を追加 | `format_table.rs` (+ 新しい形式の `.rs`) | `FORMATS` に1行 (配列の要素数も直す) |

## 命令を追加する手順

1. `emitter/inst_table.rs` の `dispatch` に `"ニーモニック" => emit_xxx(e, ops, ...)` を足す。
2. `emit_xxx` を書く。使う道具:
   - `expect_len(ops, 最小, 最大)` でオペランド数を検査する
   - `reg(ops, i)` / `imm(ops, i)` / `symbol(ops, i)` でオペランドを取り出す
   - `same_width(a, b)` でレジスタ幅の一致を検査し、`sf(rd)` で `sf` ビットを得る
   - `e.emit_u32(word)` で1語書き出す
   - ラベルを参照するときは **`emit_u32` の前に** `e.add_fixup(label, FixupKind::...)` を呼ぶ
3. 見本: オペランド無しは `emit_fixed`、3レジスタは `emit_div`、ラベル参照は `emit_branch`。
4. 命令の別名は、別の命令のオペランドを組み替えて呼ぶ (`emit_cmp` が `subs` を、`emit_tst` が `ands` を呼ぶ形)。
5. `emitter/inst_table.rs` 末尾の `CASES` に `("ソース", &[期待する命令語])` を足す。

## 符号化をテストする

期待値は GNU as で作ります (`binutils-aarch64-linux-gnu` が必要)。

```sh
# gas 側は `%` を消し、`$` を `#` にする
sed -e 's/%//g' -e 's/\$/#/g' test.s > test.gas.s
aarch64-linux-gnu-as test.gas.s -o gas.o
aarch64-linux-gnu-objcopy -O binary -j .text gas.o gas.bin
# 同じ内容をこのアセンブラで -o に通して、同じように .text を取り出して比べる
cmp gas.bin ours.bin
```

実行ファイルは `qemu-aarch64 ./a.elf` で動かせます。
`-o` で作ったオブジェクトは `aarch64-linux-gnu-ld a.o -o a.out` でリンクできます。
