# 使い方
## 変数
### スタック領域と静的領域の指定
```
var: [uint] = 0
var: static[uint] = 0
```
### 可変と不変
何もつけない場合不変変数となり
値を変えたい場合はmutをつけなければいけない
```
var: int = 0
var mut: = 0
var const: = 0
```
### ポインタと配列
1. 配列の定義方法
    ```
    var: [int 5] = {0}
    var: [int 5] = {1, 2, 3}
    ```
2. ポインタの作成方法
    ポインタの変数を[]で囲むことでメモリの先にアクセスできる
    ```
    var: [int] = 0
    ptr: int* = [var]
    [ptr] = 0
    ```
3. 配列のアクセス方法
    配列にアクセスする際に、constか即値でなければ、else ..をつけなければいけない
    これは配列の範囲外に出たときにアクセスする場所
    ```
    var: [int 3] = {0}
    [var 0] = 0
    [var idx else 0] = 0
    [var const_idx] = 0
    ```
### 演算子
#### 算術
```
var: int = 1 + 2 - 3 * 4 / 5 % 6
var: int = 1 << 2 >> 3 & 4 | 5 ^ 6
```
- += 
- -= 
- *=
- /=
#### 比較
- `<`
- `>`
- `<=`
- `>=`
- `!=`
- `==`

## 関数
### 定義方法
```
name(args: int, args2: uint): int {
    ret 0
}
```

## 構文
### 条件分岐
```
cond expr {
    ...
} else {
    ...
}

cond expr {
    1 => {}
    2 => {}
    3 => {}
    else => {}
}

cond {
    expr => {}
    expr2 => {}
    else => {}
}
```
### 反復処理
```
loop expr {
    //
}
```
## 契約
### やり方
型にmustをつけると代入さきの関数
ofをつけると契約元の関数
mustで指定した関数にわたすことで契約が終了する
もし契約した変数を契約終了させなければコンパイルエラー
契約が終了した後にその変数を使うとコンパイルエラー
```
func(): int must=func2 {
    ..
}

func2(a: int of func) {
    //
}

_start() {
    a: int must=func2 = func()
    func2(a)
}
```
## データ
### 構造体&バリアント
```
struct A {
    field: int
    field2: int

    func(self: Self): int {
        self.field ...
    }
}

variant B {
    Mem(int)
    Mem2
    Mem(i64)

    func()...
}
```