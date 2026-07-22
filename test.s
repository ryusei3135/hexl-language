.data
M0: .ascii "hello world\n"
M1: .ascii "tomato"

.text

.global _start
.extern add_1
_start:
  mov $10, %edi
  mov $1, %esi
call add
  mov $0, %eax
  mov $60, %edi
  syscall
ret
add:
L0:
  mov $0, %eax
  cmp %eax, %edi
  jg L2
jmp L1
L2:
  mov $5, %ecx
  cmp %ecx, %edi
  jg L3
  push %rax
  push %rdi
  mov $1, %eax
  mov $1, %edi
  lea M1(%rip), %rsi
  mov $10, %edx
  syscall
  pop %rdi
  pop %rax
jmp L4
L3:
  push %rax
  push %rdi
  mov $1, %eax
  mov $1, %edi
  lea M0(%rip), %rsi
  mov $12, %edx
  syscall
  pop %rdi
  pop %rax
L4:
  mov $1, %edx
  sub %edx, %edi
jmp L0
L1:
ret
