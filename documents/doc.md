# Hexl 1.0 — 構文クイックガイド

Hexl は、低レベルな制御を保ちながら、コンパイル時の検査によってメモリ安全性を目指すプログラミング言語です。C 言語フロントエンドを備え、C ソースとの連携も重視しています。

**開発状況:** Hexl 1.0 をリリースしました。現在もバグ修正とコンパイラの改善を進めています。一部の機能は実装・検証が進行中です。この文書は入門用の簡易ガイドであり、正式な仕様書ではありません。

## 1. 基本的なプログラム

```hexl
#include io="library/os/io.hexl"
_start(): int {
    msg: byte* = "hello world\n"
    io::print_line(msg, 12)
    io::exit()
}
```

- `main` は関数名です。
- `(): int` は引数がなく、`int` を返すことを表します。
- `{ ... }` は関数本体です。
- `ret` は値を返します。

## 2. 変数と型

変数は `名前: 型 = 値` の形式で宣言します。

```hexl
count: int = 10
enabled: bool = true
ratio: f64 = 1.5
```

変更可能な変数には `mut` を指定します。

```hexl
count mut: int = 10
count = count + 1
```

## 3. 関数

```hexl
add(a: int, b: int): int {
    ret a + b
}

_start(): int {
    result: int = add(2, 3)
    ret result
}
```

基本構文:

```hexl
function_name(argument: type): return_type {
    // 処理
}
```

ジェネリクス関数の例:

```hexl
identity<T>(value: T): T {
    ret value
}

result: int = identity<int>(10)
```

## 4. 式と演算子

```hexl
sum: int = a + b
difference: int = a - b
product: int = a * b
quotient: int = a / b

equal: bool = a == b
different: bool = a != b
less: bool = a < b
greater: bool = a > b
```

算術演算、比較演算、ビット演算などが利用できます。対応する演算子はコンパイラのバージョンによって確認してください。

関数呼び出し:

```hexl
result: int = add(2, 3)
```

メンバーアクセス:

```hexl
value: int = object.member
```

## 5. 条件分岐

```hexl
_start(): int {
    value: int = 10

    cond value == 10 {
        ret 1
    } else {
        ret 0
    }
}
```

パターンに応じた分岐の例:

```hexl
cond {
    1 => {
        // パターン 1
    }
    2 => {
        // パターン 2
    }
    else => {
        // その他
    }
}
```

`where` や `else` による制約処理など、一部の機能は実装状況に注意してください。

## 6. ループ

```hexl
loop {
    // 繰り返す処理
}
```

ループ制御には `break` と `continue` を使う構文があります。範囲を用いた反復などの詳細は、現在のコンパイラの対応状況を確認してください。

## 7. 構造体

```hexl
struct Point {
    x: int
    y: int
}

_start(): int {
    p: Point = Point {
        x: 10
        y: 20
    }

    ret p.x
}
```

メンバーには `object.member` でアクセスします。

## 8. バリアント

`variant` は列挙型やデータ付きバリアントを表すための機能です。

```hexl
variant Result {
    Success(int)
    Failure
}
```

構築例:

```hexl
value: Result = Result::Success(10)
```

データを持たないメンバーは、次のように表します。

```hexl
value: Result = Result::Failure
```

データ付きバリアントの値の取り出し方などは、コンパイラの対応状況を確認してください。

`variant "unsafe"` は、通常のバリアントとは異なる、共用体形式の機能です。

## 9. 配列とポインタ

配列の宣言:

```hexl
values: [int 5] = {0}
```

配列要素へのアクセス:

```hexl
first: int = [values 0]
```

範囲付きポインタの構文例:

```hexl
ptr: int[* 0..10]
```

範囲付きポインタは、ポインタが扱える範囲を型で表現するための仕組みです。ただし、範囲検査などの実装状況は機能ごとに異なります。

## 10. 契約システム

Hexl には、値に対して必要な処理や終了条件を関連付ける契約システムがあります。

構文例:

```hexl
value: int must=finish = 10
```

これは契約構文の概念例です。`must` に指定する関数や契約の成立条件には制約があります。契約システムの利用時は、最新のコンパイラで受け付けられる構文を確認してください。

## 11. `#include` — Hexl と C のファイル連携

Hexl では、Hexl ソースファイルを取り込む場合と、C ソースファイルを取り込む場合で用途が異なります。

### 11.1 Hexl ソースを取り込む

```hexl
#include "utils.hexl"
```

Hexl の `#include` は、指定した Hexl ファイルの内容を読み込み、記述した場所に展開するために使います。

たとえば、共通の関数や型定義を `utils.hexl` にまとめ、必要なファイルから取り込むことができます。

```hexl
#include "utils.hexl"

_start(): int {
    ret 0
}
```

同じファイルを複数回取り込むと定義が重複する可能性があるため、重複した宣言には注意してください。

### 11.2 C ソースを取り込む

Hexl の C 言語フロントエンドを利用すると、C ソースファイルを次の形式で指定できます。

```c
#include "file.c"
```

この形式では、C ソースを C 言語フロントエンドの対象として扱います。Hexl のソースファイルを展開する `#include` とは用途が異なります。

C 言語との連携を簡単にすることは、Hexl の重要な設計目標の一つです。実際に利用できる機能は、ビルド設定、対象プラットフォーム、C フロントエンドの対応範囲に依存します。

### 11.3 マクロの例

```hexl
#define ANSWER 42

_start(): int {
    ret #ANSWER
}
```

プリプロセッサを利用することで、定数や条件付きコンパイルなどを扱えます。詳細なマクロ構文については、実際の実装を確認してください。

## 12. 開発状況

- Hexl 1.0 をリリース済み。
- C 言語フロントエンドを実装。
- ゼロランタイムを目標とし、コンパイル時の検査によるメモリ安全性を重視。
- 現在もバグ修正とコンパイラの改善を継続中。
- 一部の機能は実装途中、または検査・コード生成の改善が必要。

## 13. まとめ

Hexl は、C 言語との連携、低レベルな制御、コンパイル時の安全性を重視するプログラミング言語です。

このガイドは基本的な構文を素早く把握するための入門資料です。正式な仕様や最新の動作については、プロジェクトのソースコードと更新されたドキュメントを優先してください。