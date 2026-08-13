# Assembly Language Format Configuration

## `asm.yaml`

```yaml
settings:
  - file: x64.yaml
    name: x64
  - file: gcc_x64.yaml
    name: gcc_x64
default: 1
entry: _start
```
1. Write a list of format files in settings.
    - file
        Specifies the name of the file under asm_fmts/.
        Include the .yaml extension.
    - name
        Specifies the name used to select the format in inline assembly.
        This is also used when selecting the format dynamically through an option.
2. default
    Specifies the index of the format to use by default.
3. entry
    Specifies the entry point when generating the output.
    asm_fmts/

Put assembly language format files in this directory. The file extension must be .yaml.

## Format
1. reg

Defines registers.

All registers must be specified as arrays.

```
db: 8-bit
dw: 16-bit
dd: 32-bit
dq: 64-bit
reg:
  db:
    - al
    - cl
    - dl
    - bl
  dw: [..]
  dd: [..]
  dq: [..]
```

2. args/fmt

Specifies the argument registers for each operating system.
```
args:
  fmt:
    linux: [4, 3, 2, 1, 5, 6]
    win: [1, 2, 5, 6]
```
3. section

Specifies the format used when defining a section.
```
section: ".{name}\n"
```
4. fmt

Specifies the format used to convert values, memory references, and other operands into strings.

Placeholders such as {} and {name} are replaced with actual values.

Key	Description	Placeholder
reg	Format for register references	{}: Register name (reg section value)
num	Format for numeric literals	{}: Number
static_var	Format for references to static variables, such as %rip-relative references	{name}: Label name
string	Format for placing string literals in the data section	{name}: Label name, {}: String contents
global	Format for exposing a symbol (.global)	{name}: Symbol name to expose
ref_stack	Format for referencing a value on the stack using an offset from %rbp	{src}: Base register (normally %rbp), {size}: Offset in bytes
get_ptr	Special format used when assigning a pointer to a value located on the stack to a variable	{size}: Offset from %rbp in bytes. The base register is always %rbp, so {src} is not used
frame	Format for creating a stack frame at the beginning of a function (push %rbp / mov %rsp, %rbp)	{space}: Indentation
frame_end	Format for releasing the stack frame at the end of a function (leave)	{space}: Indentation
data.head	Format at the beginning of a function for allocating stack space for local variables	{space}: Indentation, {size}: Number of bytes to allocate (already aligned to an 8-byte boundary)
data.fmt	Format for writing values to structure members or other data located on the stack	{space}: Indentation, {dst}: Value to write, {size}: Offset from %rbp
op_size.db/dw/dd/dq	Size suffixes appended to mnemonics, such as the l in movl	The suffix string itself (no placeholder)
```
fmt:
  reg: "%{}"
  num: "${}"
  static_var: "{name}(%rip)"
  string: "{name}: .ascii \"{}\"\n"
  global: ".global {name}\n"
  data:
    head: "{space}push %rbp\n{space}mov %rsp, %rbp\n{space}sub ${size}, %rsp\n"
    fmt: "{space}mov {dst}, -{size}(%rbp)\n"
  op_size:
    db: b
    dw: w
    dd: l
    dq: q
  ref_stack: "-{size}({src})"
  get_ptr: "-{size}(%rbp)"
  frame: "{space}push %rbp\n{space}mov %rsp, %rbp\n"
  frame_end: "{space}leave\n"
```
5. op

Defines templates for each instruction (opcode).

Each key represents the type of instruction, and its value is an object containing len and template.

Key	Corresponding operation/instruction
push	push onto the stack
pop	pop from the stack
add	Addition (+)
sub	Subtraction (-)
mul	Multiplication (*)
div	Division (/)
mov	Assignment / moving a value
cmp_l	< comparison and jump
cmp_g	> comparison and jump
cmp_e	== comparison and jump
cmp_ne	!= comparison and jump
ret	Return from a function (ret)
address	Obtaining an address (lea)
len

The number of operands required by the template.

Currently, this value is only retained as information for the code generation side. It corresponds to the number of placeholders used in the template.

template

The assembly lines to actually output.

If multiple lines are required, they are concatenated using \n.

The following placeholders can be used:

{space}: Indentation
{dst}: Destination operand
{src1} / {src2}: Source operands
{label}: Jump target label for cmp_* instructions
op:
  push:
    len: 1
    template: "{space}push {dst}\n"
  add:
    len: 2
    template: "{space}mov {src1}, {dst}\n{space}add {src2}, {dst}\n"
  cmp_l:
    len: 2
    template: "{space}mov {src1}, {dst}\n{space}cmp {src2}, {dst}\n{space}jl {label}\n"
  address:
    len: 1
    template: "{space}lea {src1}, {dst}\n"
Notes
cmp_l, cmp_g, cmp_e, and cmp_ne are keys used to look up templates. The actual comparison instruction emitted is always the cmp mnemonic.
When appending a size suffix to the mnemonic, use cmp rather than the template key such as cmp_l.
address is also a key used to look up a template. The actual mnemonic emitted is lea.
lea always uses a pointer-sized (64-bit) register and suffix.
6. func

Defines formats related to function definitions and function calls.

Key	Description	Placeholder
extern_def	Format for declaring an externally defined function (.extern)	{name}: Function name
ret	Register number where the function return value is placed (index in the dd array of reg)	No placeholder (specify the number directly)
call	Format for calling a function (call)	{name}: Function name to call
```
func:
  extern_def: ".extern {name}\n"
  ret: 0
  call: "call {name}\n"
```