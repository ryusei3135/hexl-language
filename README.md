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
- Rust による実装
- AST ベースのインタプリタ
- 静的型付け（予定）

## 目的（Motivation）
言語処理系・コンパイラ・インタプリタの学習目的で開発しています。

## 使い方
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
### 変数の宣言方法と変数に代入する方法
- 最初に<i32>(型)をつけることで代入する値の型を強制できる
- 型をつけずに宣言することもできる
- 変数に値を代入するときは、”=” を使う
```hexl
def start() {
  <i32>var := 0
  var := 0
  var = 10
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

## Changelog
- [CHANGELOG.md](./CHANGELOG.md)
