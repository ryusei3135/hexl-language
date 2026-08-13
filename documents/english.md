<p align="center">
  <img src="hexl_lang.png" width="128" />
</p>

# Hexl (Hexadecimal Language) - Custom Programming Language
Hexl is a programming language implemented in Rust. 
It features a custom-built lexer, parser, AST, and compiler, with the ultimate goal of creating a C-like programming language.
- [X64 Assembler](https://github.com/ryusei3135/hexl-language/tree/HasmX64)

## Overview
A C-like programming language.

## Features
- Custom-built programming language
- Statically typed
- Simple syntax

## Motivation
- To solve the friction/tedium encountered during everyday programming.
- To achieve an extremely minimalist syntax by reducing keywords to the bare minimum.
- To build a low-level oriented language.
- To create a C-like programming language.

## Documentation

### Usage
- Requires `gcc`
```bash
./<built-binary-name> source_code.hexl
gcc <generated-assembly-source-code> -nostdlib
```
- This will generate the executable file.
* **Note:** Currently, only Linux is supported. If you are on Windows, we highly recommend using WSL (or Docker).

### Reserved Keywords
- `ret`
- `cond`
- `loop`
- `pub`
- `const`
- `struct`
- `enum`

### Preprocessor
- `#include`
- `#asm(..)`
    [Details](./english/inline_asm.md)
    - You can use inline assembly by passing the name of any assembly configuration file into `(..)`.

### Variable Definitions
1. Standard variable definition
2. Stack-allocated variable
3. Statically-allocated variable
4. Array variable
```
d: int = 5
a:[int] = 10
b: ""[int] = 10
c: [int 4] = {1, 2, 3, 4}
```

- [Memory Specifications](./english/mem.md)
- [Variable & Array Handling](./english/variable.md)
- [Type System](./english/type.md)

### Structures & Enums
- [Structs and Enums](./english/struct_and_enum.md)

### Function Definitions

### Conditional Branching
- [Conditional Branching (Match)](./english/match.md)

### Inline Assembler
- [Assembly Language Formats](./english/asm_fmts.md)

## Changelog
- [CHANGELOG.md](../CHANGELOG.md)

## 📄 License
This project is licensed under the **[MIT License](../LICENSE.txt)**.
You are free to use, modify, and distribute it for both personal and commercial purposes.

---
© 2026 Ryuusei/Organization.