# Structures and Enumerations

## Structures

A structure is a feature that allows multiple members to be grouped together and treated as a single type.

### Definition

```text
struct Name {
    mem: int
    mem2: int
}
```

Specify the structure name after `struct`, and define its members inside `{}`.

Each member is defined in the following format:

```text
name: ty
```

### Initialization

A structure is initialized by specifying the structure name and the values of its members.

```text
a: Name = Name {
    mem: 1
    mem2: 10
}
```

---

## Structure Methods

Structures can have **methods**, which are separate from ordinary functions.

A method is a function that belongs to a structure and can be called from an instance of that structure.

### Function Definition

Ordinary functions are defined using the following syntax:

```text
name(arg: ty, ..): ty {
    ..
}
```

For example:

```text
add(a: int, b: int): int {
    return a + b
}
```

### Accessing the Instance

A structure method can use `self` to access the current instance.

```text
method(self: Self, arg: ty, ..): Self {
    ..
}
```

`Self` represents the type of the structure for which the method is currently being defined.

For example:

```text
struct Name {
    value: int

    add(self: Self, value: int): Self {
        self.value = self.value + value
        return self
    }
}
```

`self` represents the instance on which the method was called.

---

### Calling Methods

After instantiating a structure, you can access its methods using `.`.

```text
a: Name = Name {
    value: 10
}

a.add(5)
```

In this case,

```text
a.add(5)
```

calls the method with `a` as `self`.

---

## Structure Initialization Methods

A structure can define a method named `new`.

If `new` is defined, the structure can be initialized using `::new()`.

### Defining `new`

```text
new(): Self {
    ..
}
```

For example:

```text
struct Name {
    value: int

    new(): Self {
        return Self {
            value: 0
        }
    }
}
```

### Initialization Using `new`

```text
a: Name = Name::new()
```

`Name::new()` calls the `new` method defined for `Name` and creates an instance.

If `new` is not defined, the structure cannot be initialized using this method.

---

## Accessing Methods

There are two main ways to access structure methods.

### Accessing from the Type

Use `::` to access a method that belongs to a structure.

```text
Name::new()
```

This calls a method directly from the structure type.

It can be used for initialization or other operations that do not require an instance, such as `new`.

### Accessing from an Instance

Use `.` to access a method from an instantiated structure.

```text
a.func()
```

For example:

```text
a: Name = Name::new()

a.func()
```

In this case, `a` is passed to the method as `self`.

---

## Enumerations

An enumeration is a feature that allows multiple choices to be represented as a single type.

### Definition

```text
enum Name {
    Mem
    Mem2
}
```

Specify the enumeration name after `enum`, and define its members inside `{}`.

In this example, the `Name` type has two values: `Mem` and `Mem2`.

### Initialization

Enumeration values are specified using `::`.

```text
a: Name = Name::Mem
```

`Name::Mem` represents the `Mem` value of the `Name` type.

---

## Differences Between Structures and Enumerations

Both structures and enumerations are features for defining new types, but their method-related specifications are different.

| Feature | Structure | Enumeration |
| --- | :-: | :-: |
| Has members | ○ | ○ |
| Can be instantiated | ○ | ○ |
| Can define methods | ○ | × |
| Can use `self` | ○ | × |
| Can use `Self` | ○ | × |
| Supports `::new()` | ○ | × |
| Supports method access using `.` | ○ | × |

**Only structures can define methods.**

Therefore, structures can use access patterns such as:

```text
Name::new()
a.func()
```

while enumerations cannot define methods.
