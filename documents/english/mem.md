# Memory Access

In this language, `[]` can be used to directly access memory through variables or addresses.

Memory access provides the following features:

- Specifying the type stored in memory
- Placing variables in memory regions
- Placing variables in static memory
- Creating pointers
- Reading from and writing to memory using pointers
- Reading from and writing to arrays
- Direct memory access using `[]`

---

## Defining the Type Stored in Memory

**When defining a variable, a value must always be provided.**

When storing a value in memory, the type to be stored is specified using the `[T]` syntax.

```text
var: [T] = value
```

`T` specifies the type of the value to be stored in memory.

For example, to store an `int` value in memory:

```text
var: [int] = 0
```

With this information, the compiler can determine the size and interpretation of the data when reading from or writing to memory.

---

## Placing Variables in a Memory Region

When a variable is declared normally, it is basically placed in the **stack region**.

```text
stack: [T] = value
```

For example:

```text
stack: [int] = 0
```

This allocates space on the stack to store an `int` value.

If no specific placement is specified, the stack region is used as the default location.

---

## Variables in Static Memory

To place a variable in static memory, use `""[T]`.

```text
static: ""[T] = value
```

For example:

```text
static: ""[int] = 0
```

This specifies that an `int` value should be placed in static memory.

Normally,

```text
var: [int]
```

places the variable in the stack region, while explicitly specifying the memory region as:

```text
var: ""[int]
```

places the variable in static memory.

Data placed in static memory remains in a fixed memory region throughout program execution, independently of individual function invocations.

---

## Creating Pointers

A pointer can be declared by adding `*` to a type.

```text
a: int* = [var]
```

In this example, `a` is a pointer to an `int` value.

`[var]` is used to obtain the memory address of the target variable.

In other words,

```text
a: int* = [var]
```

means:

```text
a ← address of var
```

---

## Reading and Writing Memory Using Pointers

Enclosing a pointer in `[]` allows access to the memory pointed to by that pointer.

### Reading

```text
a: int* = [var]

value: int = [a]
```

`[a]` reads a value from memory using the address stored in `a`.

### Writing

```text
a: int* = [var]

[a] = 10
```

In this case, `10` is written to the memory pointed to by `a`.

---

## Memory Access Using `[]`

In this language, an expression enclosed in `[]` is treated as a **memory access expression**.

```text
[expression]
```

The address represented by `expression` is used to access the corresponding memory.

For example:

```text
[a]
```

uses the address stored in `a`.

To read a value from memory:

```text
value = [a]
```

To write a value to memory:

```text
[a] = value
```

This syntax clearly distinguishes ordinary variable access from direct memory access.

---

## Reading and Writing Arrays

For arrays, a specific element can be accessed by specifying an index.

```text
[var index]
```

`var` is the array, and `index` is the index of the element to access.

For example,

```text
[var 0]
```

accesses the first element of the array.

```text
[var 2]
```

accesses the third element of the array.

### Reading

```text
value: int = [var index]
```

This reads the element at the specified index.

### Writing

```text
[var index] = 10
```

This writes `10` to the element at the specified index.

---

## Memory Regions

The placement of a variable is determined by how it is declared.

| Declaration | Placement |
| --- | --- |
| `var: [T]` | Stack region |
| `var: ""[T]` | Static region |
| `a: T* = [var]` | Obtains the address of `var` |

A normal variable without an explicitly specified placement is placed in the stack region.

---

## The Concept of Memory Access

`[]` is not merely syntax dedicated to array access. It is the **fundamental syntax for accessing memory**.

Therefore, syntax such as:

```text
[a]
```

and:

```text
[var index]
```

are both essentially handled by the same mechanism: **accessing the memory at a specified location**.

This allows pointers, arrays, stack memory, static memory, and other memory-related features to be handled through a unified memory access mechanism.

---

## Syntax Reference

| Syntax | Purpose |
| --- | --- |
| `[T]` | Specifies the type stored in memory |
| `var: [T]` | Places a variable in the stack region |
| `var: ""[T]` | Places a variable in the static region |
| `a: T*` | Pointer to type `T` |
| `[var]` | Obtains the address of a variable |
| `[ptr]` | Accesses the memory pointed to by a pointer |
| `[var index]` | Accesses an array element |
| `[expr]` | Accesses the memory represented by an expression |

---

## Summary

Memory access in this language is designed around the `[]` syntax.

```text
var: [T]        Place in the stack region
var: ""[T]      Place in the static region
[var]           Obtain an address
[ptr]           Access through a pointer
[var index]     Array access
```

If no placement is specified, the stack region is used by default. Other placement methods, such as static memory, can be specified when needed.

By using a unified `[]` syntax, the language can directly express memory access operations ranging from array access to pointer manipulation, providing a direct representation of the memory access capabilities provided by the CPU.
