# Hexl Syntax Quick Guide

A beginner-friendly introduction to Hexl's syntax and core features.

## 1. Basic Program Structure

A simple Hexl program looks like this:

```hexl
_start(): int {
    ret 0
}
```

- `_start()` is the entry-point function.
- `: int` specifies the return type.
- `ret` returns a value.

## 2. Variables

Declare a variable by specifying its name, type, and initial value.

```hexl
count: int = 10
```

A mutable variable can be declared using `mut`:

```hexl
count mut: int = 10
count = 20
```

Hexl uses explicit type annotations to make code easier to understand and allows the compiler to perform type checking.

## 3. Functions

Define a function by specifying its name, parameters, and return type.

```hexl
add(a: int, b: int): int {
    ret a + b
}
```

Call the function:

```hexl
result: int = add(10, 20)
```

### Generic Functions

Hexl also supports generic function syntax.

```hexl
identity<T>(value: T): T {
    ret value
}
```

The type parameter `T` allows the function to work with different types.

## 4. Expressions and Operators

Hexl supports arithmetic and comparison operations.

```hexl
a: int = 10 + 5
b: int = a * 2
c: bool = a == b
```

Common operators include:

| Category | Operators |
|---|---|
| Arithmetic | `+`, `-`, `*`, `/`, `%` |
| Comparison | `==`, `!=`, `<`, `>`, `<=`, `>=` |
| Assignment | `=` |

The available operators depend on the current implementation and type rules.

## 5. Conditional Branching

Use `cond` to write conditional branches.

```hexl
cond value == 10 {
    value = 20
} else {
    value = 0
}
```

You can also use `cond` to distinguish between different cases.

```hexl
cond value {
    0 {
        value = 1
    }
    1 {
        value = 2
    }
    else {
        value = 0
    }
}
```

Note: Some conditional features, particularly those involving `where` and `else`, may still be under development.

## 6. Loops

Use `loop` to express repeated execution.

```hexl
loop {
    // Repeated statements
}
```

A range-based loop can be written as follows:

```hexl
loop i = 0..10 {
    // Process i
}
```

The exact syntax and supported loop forms depend on the current implementation.

## 7. Structures

Use a structure to group related data.

```hexl
Point {
    x: int
    y: int
}
```

Create a value:

```hexl
p: Point = Point {
    x: 10
    y: 20
}
```

Access a member:

```hexl
x: int = p.x
```

## 8. Variants

Hexl uses `variant` to represent values that can take different forms.

```hexl
variant Result {
    Success(int)
    Failure
}
```

Construct a variant value:

```hexl
result: Result = Result::Success(10)
```

Variants can represent alternatives such as successful and failed results. The supported construction and access syntax depends on the implemented variant features.

## 9. Arrays and Pointers

### Arrays

Declare an array with a specified element type and length:

```hexl
values: [int 5] = {0}
```

Access an element using the bracket syntax:

```hexl
value: int = [values 0]
```

### Range-Bounded Pointers

Hexl is designed to support pointers with explicit range constraints.

```hexl
ptr: int[* 0..10]
```

The intended purpose is to help the compiler check pointer operations against permitted ranges. The level of range checking depends on the current implementation.

## 10. Contracts

Hexl's contract system associates values with obligations that must be fulfilled.

For example:

```hexl
value: int must=finish = 10
```

Here, `must=finish` associates the value with the designated contract-ending function, `finish`.

The contract system is intended to help detect unresolved obligations and unsafe resource handling at compile time, reducing the need for runtime checks.

## 11. Preprocessor and Source Inclusion

Hexl provides preprocessor functionality.

### Including Another Hexl Source File

```hexl
#include "utils.hexl"
```

This includes the specified Hexl source file at the inclusion point.

### Including C Source Code

```hexl
#include "file.c"
```

Hexl's C frontend allows C source code to be incorporated into the compilation workflow. This is intended to make interoperability with C libraries and embedded development easier.

### Defining a Macro

```hexl
#define ANSWER 42

_start(): int {
    ret #ANSWER
}
```

The preprocessor replaces the macro reference with its defined value.

## 12. Development Status

Hexl 1.0 has been released, and the C frontend has been implemented. Bug fixes and further improvements are ongoing.

Some advanced features may still be incomplete or subject to change. Check the project's source code and documentation for the latest implementation details.

## 13. Summary

Hexl combines explicit types, compile-time checking, contracts, and C interoperability.

Its design aims to provide beginner-friendly abstractions while enabling advanced control over memory and low-level programming.

For the latest updates and source code, visit the [Hexl GitHub repository](https://github.com/ryusei3135/hexl-language).
