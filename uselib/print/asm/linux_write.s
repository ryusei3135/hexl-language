.text

.global linux_sys_write
.type linux_sys_write, @function

linux_sys_write:
    movq %rsp, %rbp

    subq $16, %rsp

    movq $1, %rax
    movq %rsi, %rdx
    movq %rdi, %rsi
    movq $1, %rdi

    syscall

    leave
    ret