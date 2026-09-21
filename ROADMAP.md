# 予定されている機能

## 構造体
```
struct A {
    func(a: int) {}
}

struct A::func(a: int) {
    func2() {}
}
```
---
```
static struct A {
    func(): int
}

struct B(A) {
    func(): int {
        ret 10 + 10
    }
}
```
---
```
struct A: register {
    a: int
    b: int
}
```
---
```
struct A<T> {
    a: T
}
```

## 契約
```
where=0..10 var: int = 5
```
---
```
struct: must A {
    a: int must=f
}
```
---
```
where=0..10 var: int = 5
```