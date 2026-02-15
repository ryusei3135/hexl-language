<p align="center">
  <img src="hexl_lang.png" width="128" />
</p>

# HexlLanguage - 自作プログラミング言語 / Custom Programming Language
Hexlはrustで実装されたプログラミング言語であり
字句解析・構文解析・AST・インタプリタを自前で実装しており、
自分で使うことと学習を目的としています

## 概要（Overview）
このプロジェクトは私が使うプログラミング言語を作る

## 特徴（Features）
- 自作プログラミング言語
- AST ベースのインタプリタ
- 静的型付け
- シンプルな構文

## 目的（Motivation）
言語処理系・コンパイラ・インタプリタの学習目的で開発しています。

## ビルド方法（Docker）
本プロジェクトには `Dockerfile` が含まれています。
以下の手順で Docker イメージをビルドし、環境を構築してください。

Linux / macOS（bash / zsh）
```bash
docker build -t hexl .
docker run -it --rm -v $(pwd)/:/workspace -w /workspace hexl
cargo run -- sample/fizzbizz.hexl
```
Windows（PowerShell）
```PowerShell
docker build -t hexl .
docker run -it --rm -v ${PWD}:/workspace -w /workspace hexl
cargo run -- sample/fizzbizz.hexl
```

## 使い方
- サンプルコードは、プロジェクトの"sample/"にあります
- 処理はstart関数の中に書く
  start関数から実行される
### 文字列の出力の仕方
- 標準ライブラリのstd::ioを使うことで、文字列を出力することができる
```hexl
use std::io

def start() {
  io::print("Hello world")
}
```
### 変数
#### 定義の仕方
- 最初に<i32>(型)をつけることで代入する値の型を強制できる
- 型をつけずに宣言することもできる
- 変数に値を代入するときは、”=” を使う
```hexl
def start() {
  <i32>var := 0
  mut var1 := 0
  var1 = 10
}
```
#### 不変と可変
- "mut"
  これをつけることで、可変変数にすることができる
- "imm"
  これをつけることで、明示的に不変であることを示すことができる
- 何もつけない場合
  デフォルトで不変
- loop文や引数などでも有効
```hexl
def start() {
  mut var := 0
  imm var1 := 0
  var2 := 0
}
```

### 関数の宣言と、引数、戻り値
- 引数の型も戻り値もこのように<>の中に入れた型になります
- retの右に値を書くことで、値を返すことができる
- 戻り値を書かないと何も返せない
```hexl
def add(<i32>a, <i32>b)<i32> {
  ret a + b
}
```

### 条件分岐
- if文は、rustライクな感じに書けます
- if else文は、"if else"と書かなくても使える
```hexl
use std::io

def start() {
  a := 0
  if a == 5 {
    io::print("a is 5")
  } a == 3 {
    io::print("a is 3")
  } else {
    io::print("other")
  }
}
```

### loop文
- loop文は、変数に代入する処理を書かずに動かせます、
- loop文に5だけ入れた場合は 0~5まで、
- a..bは今のところ、aにマイナスの値を入れることができます
```hexl
use std::io

def start() {
  loop 5 {
    io::print("a")
  }
  loop 0..5 {
    io::print("a")
  }
  loop a in 0..5 {
    io::print("b")
  }
}
```

## Changelog
- [CHANGELOG.md](./CHANGELOG.md)
