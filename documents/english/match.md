# About `cond` Expressions

`cond` is an expression used for conditional branching.

There are three ways to use `cond`, depending on what is provided to it:

- **Provide a Boolean value**
  - Performs a simple conditional branch similar to `if`.
- **Provide a value**
  - Compares the value against each pattern and executes the matching branch.
  - This is similar to `switch` or `match` in other programming languages.
- **Provide nothing**
  - Evaluates each condition from top to bottom and executes the first branch whose condition is true.
  - This can be used similarly to a sequence of `if` / `else if` / `else` statements.

## 1. Providing a Boolean

If a Boolean expression is placed after `cond`, the branch is selected based on its result.

```text
cond a == 10 {
    // when a is 10
} | {
    // otherwise
}
```

This has the same meaning as the following `if` statement:

```text
if (a == 10) {
    //
} else {
    //
}
```

Using a `Boolean` is suitable for simple two-way conditional branching.

## 2. Providing a Value

If a value is placed after `cond`, that value is compared against each pattern.

```text
cond a {
    10 => {
        // a == 10
    }
    20 => {
        // a == 20
    }
    | => {
        // no match
    }
}
```

This is roughly equivalent to:

```text
if (a == 10) {
    //
} else if (a == 20) {
    //
} else {
    //
}
```

Use this form when you want to compare a value against multiple possible values.

## 3. Providing Nothing

If nothing is placed after `cond`, each block contains its own condition.

```text
cond {
    a == 10 => {
        //
    }
    a > 5 => {
        //
    }
    b == 0 => {
        //
    }
    | => {
        //
    }
}
```

Each condition is evaluated from top to bottom, and only the first block whose condition is true is executed.

This behaves similarly to the following sequence of `if` / `else if` statements:

```text
if (a == 10) {
    //
} else if (a > 5) {
    //
} else if (b == 0) {
    //
} else {
    //
}
```

This form is useful when you want to combine complex conditional branches into a single `cond` expression.

## Usage Guide

| Syntax | Use | Equivalent in Other Languages |
| --- | --- | --- |
| `cond condition` | Two-way branching | `if` |
| `cond value` | Value matching | `match` / `switch` |
| `cond` | Evaluate conditions in order | `if` / `else if` / `else` |
