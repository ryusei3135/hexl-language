.text
.global change_byte_chr
.type change_byte_chr, @function

.global is_byte_digit
.type is_byte_digit, @function

.extern peek
.type peek, @function

change_byte_chr:
    pushq %rbp
    movq %rsp, %rbp
    subq $16, %rsp

    movq %rdi, (%rsp)

    call peek

    
    movzbq %al, %rcx           # %raxの下位1バイト(value)を %rcx にゼロ拡張でコピー
    
    # \\n
    cmp $110, %rcx
    jnz .N0
    mov $10, %rax           # \\n (10) を戻り値にする
    leave
    ret
    
.N0: # \\t
    cmp $116, %rcx
    jnz .N1
    mov $9, %rax            # \\t (9) を戻り値にする
    leave
    ret
    
.N1: # \\r
    cmp $114, %rcx
    jnz .N2
    mov $13, %rax           # \\r (13) を戻り値にする
    leave
    ret
    
.N2:
    mov %rcx, %rax          # マッチしなかった場合は、読み込んだ文字コードをそのまま返す
    leave
    ret


is_byte_digit:
    pushq %rbp
    movq %rsp, %rbp
    subq $16, %rsp
    
    movzbl  %dil, %eax
    # 48以上57以下の場合のみ1を返す
    cmpl $48, %eax
    jl .Err
    cmpl $57, %eax
    jg .Err
    movl $1, %eax
    leave
    ret
.Err:
    movl $0, %eax
    leave
    ret
