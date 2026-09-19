# Type System

## Standard Types

1. `byte`
2. `u16`
3. `int`
4. `u64`

Adding `*` to a type makes it a pointer type.

```text
a: int* = ..
```

---

## Contracts

- [Contracts](./constract.md)

---

## Bounded Pointers

A bounded pointer is a feature that manages, at compile time, the range of memory that a pointer can access.

An ordinary pointer holds only an address, whereas a bounded pointer holds the range it is allowed to handle in addition to its position.

This allows out-of-range accesses caused by pointer arithmetic or memory access to be detected at compile time.

### Basic Concepts

The range of a bounded pointer is expressed by three positions.

```text
begin <= position <= end
```

Here,

- `begin` is the start of the range
- `position` is the current position of the pointer
- `end` is the end of the range

However, **memory can be accessed only at positions before `end`**.

```text
begin <= position < end
```

Therefore, although `end` is a boundary position that is considered to be within the range, memory access cannot be performed at the `end` position.

### Out-of-Range Movement

When a pointer is moved, the position after the move must be within the range.

```text
begin <= position <= end
```

For example, consider an array with five elements.

```text
a: [int 5]
```

The range of this array is managed as:

```text
0 <= p <= 5
```

Access to an element must satisfy:

```text
0 <= p < 5
```

Therefore, the pointer can be moved to the position just past the fifth element, but values cannot be read from or written to that position.

### Pointer Arithmetic

Addition and subtraction on a bounded pointer are permitted only when the compiler can confirm that the position after the move is within the range.

```text
p + 1
p - 1
```

For operations such as these, the result must satisfy:

```text
begin <= position <= end
```

If it is found at compile time that the result would be out of range, a compile error occurs.

Operations are also not permitted when the compiler cannot prove that the result is within the range, because they could lead to out-of-range access.

### Mutable Pointers

A mutable pointer is a pointer whose own position can be changed.

The following operations are permitted on a mutable pointer:

```text
p += 1
p -= 1
```

The position after the move must be within the range.

The number of mutable pointers that can exist at the same time is restricted.

In principle, at most one mutable pointer can exist for the same range.

### Immutable Pointers

An immutable pointer is a pointer that does not directly change itself.

With an immutable pointer, expressions such as:

```text
p + 1
p - 1
```

do not modify the original pointer, but instead produce a new expression representing a new position.

Operations that change the position of the immutable pointer itself are not permitted.

Operations such as:

```text
p += 1
```

cannot be used.

### Copying Immutable Pointers

An immutable pointer can be copied multiple times.

```text
a = p
b = p
c = p
```

As shown above, multiple immutable pointers can be created from the same immutable pointer.

Because an immutable pointer has no authority to modify memory, the existence of multiple immutable pointers does not cause conflicts between mutable accesses.

### Copying Mutable Pointers

Ordinary copying of a mutable pointer is not permitted.

This is because duplicating a mutable pointer would result in multiple mutable pointers that can modify the same memory range.

Creating another mutable pointer from a mutable pointer requires an explicit operation.

```text
b = *mut p
```

This operation transfers the mutability of the mutable pointer.

The original `p` can no longer be used as a mutable pointer afterward.

### `move` and Mutable Pointers

The ownership of a mutable pointer can be transferred using `move`.

```text
b = move p
```

In this case, the authority as a mutable pointer moves to `b`.

The source `p` cannot be used as a mutable pointer.

### Immutable References

An immutable reference can be created from a bounded pointer.

```text
b: int& = p
```

An immutable reference can read the target memory, but cannot modify it.

An immutable reference can be created even while a mutable pointer exists.

However, an immutable reference does not change the range or authority of the mutable pointer.

### Mutable References

To perform mutable access, use a mutable reference.

```text
&mut
```

is used as the mutable reference.

While a mutable reference exists, it is managed so that it does not conflict with any other mutable access to the same target.

### Splitting Pointers

A bounded pointer can split its accessible range.

For example, a range such as:

```text
begin ---------------- end
```

can be split into two:

```text
begin -------- split -------- end
```

Each part can then be treated as an independent range.

After the split, each pointer can operate only on the range assigned to it.

This prevents the same memory region from being modified simultaneously through multiple mutable pointers.

### The Original Pointer After Splitting

When a pointer is split, the original pointer can no longer be used as a mutable pointer.

The original pointer can only be used as an immutable reference.

```text
p
 ↓
┌────────────────────┐
│       memory       │
└────────────────────┘

split

p1                  p2
│                   │
├────────┐ ┌────────┤
         │ │
      separate ranges
```

Each of the split ranges is treated as an independent mutable pointer.

### Merging Split Ranges

Split ranges can be merged again if certain conditions are met.

After merging, the result can be treated as a pointer with the larger range it had before the split.

However, the order in which split ranges are merged cannot be changed.

This is so that the range can be reconstructed while preserving adjacency.

If:

```text
A | B | C
```

is split, the parts must be merged in the original order:

```text
A + B + C
```

It is not possible to merge them in a different order, such as:

```text
A + C + B
```

### Assigning Ranges

The range held by an immutable pointer can be changed to a different range if certain conditions are met.

However, it must be possible to confirm at compile time that the new range is also a range the pointer is allowed to access.

For mutable pointers, a simple range change is not permitted, because the consistency between mutable access authority and the range must be maintained.

### Range and Access

Memory access through a bounded pointer must satisfy:

```text
begin <= position < end
```

For example,

```text
p = end
```

is a valid move, but:

```text
*p
```

is not permitted.

This is so that `end` can be used as a boundary indicating the end of an array.

### Range and Contracts

Contract-bound pointers and bounded pointers can be combined.

```text
p: int* must free
```

When range information is attached to a contract like this, the range information is also maintained for as long as the contract is in effect.

When the contract is terminated, pointers and references that depend on that contract can no longer be used.

### Principles of Bounded Pointers

Bounded pointers are based on the following principles:

1. A pointer has an accessible range.
2. A pointer must be moved within `begin <= position <= end`.
3. Memory access must be within `begin <= position < end`.
4. Pointer arithmetic that goes out of range is not permitted.
5. Operations that the compiler cannot prove to be within range are not permitted.
6. In principle, at most one mutable pointer can exist for the same range.
7. Immutable pointers can be copied multiple times.
8. Duplicating a mutable pointer requires an explicit operation.
9. The authority of a mutable pointer can be transferred using `move`.
10. When a pointer is split, the original pointer can no longer be used as a mutable pointer.
11. Split ranges are treated as independent ranges.
12. When split ranges are merged, the original order must be preserved.
13. Range information is used to statically check pointer safety.
