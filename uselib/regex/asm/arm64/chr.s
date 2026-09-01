// AArch64 (ARM64) 版
// x86-64版にあった2つのバグを修正した上で移植している:
//   1) `movq %rdi, (%rsp)` がリターンアドレスを破壊していた -> 削除(this の保存は不要)
//   2) `call peek` になっていて位置が進まなかった -> `bl bump` に修正
//      (呼び出し元は既にバックスラッシュを bump 済みで、エスケープ文字自体を
//       消費するのはこの関数の責任のため)
//   また、プロローグ無しで `leave` していた不整合も、フレームごと整理した。

.text
.global change_byte_chr
.type change_byte_chr, %function

.global is_byte_digit
.type is_byte_digit, %function

.extern bump
.type bump, %function

change_byte_chr:
    // x0 = Parser* (this)
    stp     x29, x30, [sp, -16]!   // bl で LR が壊れるので待避
    mov     x29, sp

    bl      bump                   // 現在の文字を読み、同時に位置を1つ進める
    uxtb    w1, w0                 // CharOpt.value (下位1バイト) を取り出す

    cmp     w1, #110               // 'n'
    b.ne    .Cbc0
    mov     w0, #10                // \n (10) を戻り値にする
    b       .Cbc_ret
.Cbc0:
    cmp     w1, #116               // 't'
    b.ne    .Cbc1
    mov     w0, #9                 // \t (9) を戻り値にする
    b       .Cbc_ret
.Cbc1:
    cmp     w1, #114               // 'r'
    b.ne    .Cbc2
    mov     w0, #13                // \r (13) を戻り値にする
    b       .Cbc_ret
.Cbc2:
    mov     w0, w1                 // マッチしなかった場合は、読み込んだ文字コードをそのまま返す
.Cbc_ret:
    ldp     x29, x30, [sp], 16
    ret


is_byte_digit:
    // w0 = 第1引数 (文字コード)。呼び出しが無いリーフ関数なのでフレームは不要。
    uxtb    w1, w0                 // movzbl %dil, %eax に相当

    cmp     w1, #48
    b.lt    .Err
    cmp     w1, #57
    b.gt    .Err
    mov     w0, #1
    ret
.Err:
    mov     w0, #0
    ret