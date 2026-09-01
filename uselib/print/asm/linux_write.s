.text

.global linux_sys_write
.type linux_sys_write, @function


linux_sys_write:
    pushq %rbp
    movq %rsp, %rbp

    pushq %rdi          # [rbp-8]  に fd を保存
    pushq %rsi          # [rbp-16] に buf を保存
    pushq %rdx          # [rbp-24] に count を保存

.loop:
    # 終了条件: 残りバイト数(count)が 0 以下なら終了
    cmpq $0, -24(%rbp)
    jle .end

    # sys_write (rax=1) の引数をセット
    movq $1, %rax       # システムコール番号 (sys_write)
    movq -8(%rbp), %rdi # 第1引数: ファイルディスクリプタ
    movq -16(%rbp), %rsi# 第2引数: バッファポインタ
    movq -24(%rbp), %rdx# 第3引数: 残り書き込みバイト数

    syscall

    # エラーチェック: 返り値(rax)が 0 未満ならエラー終了
    cmpq $0, %rax
    jl .end

    # ポインタと残りバイト数の更新
    addq %rax, -16(%rbp)# buf += 書き込めたバイト数
    subq %rax, -24(%rbp)# count -= 書き込めたバイト数
    jmp .loop

.end:
    leave
    ret

