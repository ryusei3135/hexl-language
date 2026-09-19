# Using Inline Assembly

## Usage

Inline assembly is written using the `#asm` preprocessor directive.

The assembler name is one of the names configured in `asm_fmts/` or `asm.yaml`.

```text
#asm(assembler_name) {
    ..
}
```

---

## Embedding Features

The following embedding features can be used inside an inline assembly block.

- `{space}` inserts whitespace when the block is expanded.
- `${..}` allows a variable to be substituted inside the braces.

```text
#asm(gcc_x64) {
    "{space}movl ${a}, %eax"
}
```

---

## Expansion Behavior

- When inline assembly is expanded, the embedding features are processed first, and the result is then expanded as-is.
- Before the inline assembly is expanded, the registers currently in use are pushed. When the inline assembly finishes, they are popped.

---

## See Also

- [Assembly Language Format Configuration](./asm_fmts.md)
