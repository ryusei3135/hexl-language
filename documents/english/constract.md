# Contracts

A contract is a feature that attaches a constraint to a value at compile time: the value must be handled according to certain rules until a specific operation has been performed on it.

Contracts are used in particular to prevent **forgetting to free or clean up** dynamically allocated memory and similar resources.

Because contracts are checked at compile time, no runtime management mechanism is required.

---

## Basic Syntax

A contract is written after a type, using `must` followed by a terminating function.

```text
a: int* must=free = malloc(...)
```

In this example, `a` must have its contract terminated by `free`.

```text
free(a)
```

When `free(a)` is executed, the contract attached to `a` is terminated.

If the value goes out of scope without its contract being terminated, a compile error occurs.

---

## Purpose of Contracts

Contracts are mainly used to guarantee, at compile time, that processing such as the following is completed:

- Freeing memory
- Releasing resources
- Performing specific cleanup
- Consuming an owned value
- Processing by a specific function

For example, a function that allocates memory can return a pointer with a contract attached.

```text
func malloc(...): int* must=free
```

When the caller receives the return value of this function, it must pass that value to `free`.

```text
a: int* = malloc(...)

free(a)
```

If the contract is not terminated, code such as:

```text
a: int* = malloc(...)
```

results in a compile error.

---

## Terminating a Contract

A contract is terminated by passing the value to the function specified in the contract.

```text
a: int* must free = malloc(...)

free(a)
```

The function that terminates a contract receives the value under contract as an argument.

When the termination processing is complete, the contract attached to that value ends.

---

## `of`

For a function that terminates a contract, `of` can be used to specify which contract is to be terminated.

```text
func free(a: int* of free)
```

`of` indicates which contract is terminated by that argument.

For example, in:

```text
a: int* must=free = malloc(...)

free(a)
```

the argument `a` of `free` terminates the `free` contract of `a` itself.

---

## Values Under Contract

Values with an active contract are subject to constraints that differ from those of ordinary values.

A contract is maintained until the processing required for that value is complete.

### Reassignment

Reassigning a variable that is under contract is prohibited.

```text
a: int* must=free = malloc(...)

a = ...
```

This code results in a compile error.

Overwriting the value under contract with another value would lose the target whose original contract needs to be terminated.

Once the contract has been terminated, ordinary reassignment becomes possible.

```text
a: int* must=free = malloc(...)

free(a)

a = ...
```

---

## `move`

When a value under contract is moved elsewhere using `move`, the contract is moved along with it.

However, it is prohibited to `move` the value under contract in such a way that the contract can no longer be terminated, that is, before calling the terminating function specified in the contract.

For example, a contract cannot be abandoned by doing the following:

```text
a: int* must=free = malloc(...)

b = move a
```

A value under contract must eventually be passed to the specified terminating function.

---

## Contracts and References

A reference can be created from a value under contract.

However, a reference cannot move or terminate the contract itself.

For example, an immutable reference such as the following can be created:

```text
a: int* must free = malloc(...)

b: int& = a
```

This reference does not own the contract of `a`.

Therefore, when the contract of `a` is terminated by:

```text
free(a)
```

references related to it also become unusable.

---

## Contracts and Pointers

A contract can be attached to a value of a pointer type.

```text
a: int* must free = malloc(...)
```

The pointer itself becomes the target of the contract.

What can be created from a pointer under contract is restricted.

- The pointer's position
- The value the pointer points to
- Immutable references

Only operations that do not destroy the original contract, such as those above, are permitted.

Using a pointer under contract to create another ownership is prohibited.

---

## Contracts and Mutable References

When a mutable reference is created from a value under contract, it must be ensured that the mutable reference cannot cause the value under contract to be lost.

A value under contract must always remain traceable until its contract is terminated.

---

## Contract Lifetime

A contract is terminated by either of the following:

1. The specified terminating function is called.
2. The contract of the structure or other value that holds the contract is terminated.

If a variable under contract goes out of scope while its contract has not been terminated, a compile error occurs.

```text
{
    a: int* must free = malloc(...)
}
```

In this case, an error occurs because there is no `free(a)`.

---

## Nested Scopes

A contract can move across scopes together with the ownership state of the value.

However, when the contract is terminated, references and other values created from that contract also become invalid.

```text
{
    a: int* must free = malloc(...)

    {
        b: int& = a
        ...
    }

    free(a)
}
```

In this case, `b` does not own the contract of `a`.

---

## Structures and Contracts

A contract can be attached to a structure itself.

```text
struct A: must {
    p: int* must=free
}
```

When a structure has a contract, the contract applies to the structure as a whole.

Rather than managing the contract targets inside the structure individually, the structure itself is treated as the contract target.

Having multiple contract targets in a single structure is restricted.

---

## Arrays and Contracts

When values under contract are stored in an array, all contracts in the array must be trackable.

Discarding an array that contains values under contract as-is is prohibited.

For example, in an array such as:

```text
a: [int* must=free 5]
```

a contract exists for each element of the array.

Therefore, all contracts must be terminated before the array is discarded.

```text
must const i = 0..a {
    free([a i])
}
```

Once the contracts of all elements have been terminated, the array can be discarded.

---

## Principles of Contracts

The contract feature is based on the following principles:

1. Contracts are checked at compile time.
2. A value under contract must always undergo the specified termination processing.
3. A variable under contract must not be reassigned.
4. A contract cannot be lost through `move`.
5. A reference created from a value under contract does not own the contract.
6. When a contract is terminated, references that depend on it cannot be used.
7. If a contract remains at the end of a scope, a compile error occurs.
8. Contracts ensure safety through static checking at compile time, not through runtime garbage collection.
