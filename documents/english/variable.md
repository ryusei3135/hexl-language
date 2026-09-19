# Defining and Using Variables

## Defining Variables

There are four ways to define a variable.

1. Standard variable definition
2. Stack-allocated variable
3. Statically-allocated variable
4. Array variable

```text
d: ty = value
a: [int] = 10
b: static[int] = 10
c: [int 4] = {1, 2, 3, 4}
```

Variables are defined in the following format:

```text
name: type = value
```

---

## Placing Variables in Memory

- Enclosing a type in `[]` forces the variable to be placed in the stack region.
- Placing `static` before `[]` places the variable in the static region.

```text
a: [int] = 10
b: static[int] = 20
```

For details, see [Memory Access](./mem.md).

---

## Arrays

### Syntax

- As in `a` below, specifying a type and a length inside `[]` creates an array in the stack region.
- As in `b` below, initializing the array with a single value sets every element of the array to that value.

```text
a: [int 3] = {0, 1, 2}
b: [int 5] = {0}
```

### Reading and Writing

Enclose the variable in `[]` and write the element index to its right to read or write that element.

```text
[a 0] = 10
c: int = [a 0]
```

### Arrays Inside Structures

To access an array that is a member of a structure, use the form `.[name index]`.

```text
struct Name {
    arr: [int 5]
}
arr: Name = Name {
    arr: {0, 1, 2, 3, 4}
}
arr.[arr 0] = 10
```

---

## Pointers

- Enclosing a variable in `[]` obtains its address.
- Adding `*` to a type makes it a pointer type.

```text
a: int = 10
b: int* = [a]
```

- Adding `[]` to a pointer variable lets you change the value at the location the pointer points to.
- Without `[]`, the address itself is changed.

```text
[a] = 10
```
