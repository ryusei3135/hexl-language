

<p align="center">
  <img src="hexl_lang.png" width="128" />
</p>

# Hexl(Hexadecimal Language) - 自作プログラミング言語 / Custom Programming Language
Hexlはrustで実装されたプログラミング言語であり
字句解析・構文解析・AST・コンパイラを自前で実装しており、
c言語ライクな言語を作ることを目的としています。
- [X64アセンブラ](https://github.com/ryusei3135/hexl-language/tree/HasmX64)


## **english ver**
[english doc](./documents/english.md)

## 概要（Overview）
c言語ライクな言語

## 特徴（Features）
- 自作プログラミング言語
- 静的型付け
- シンプルな構文

## 目的（Motivation）
- プログラミングをしているとめんどくさくなることがあるから
- 極限までキーワードを減らしシンプルな構文
- 低レイヤ寄りの言語作成
- c言語ライクな言語の作成

## 説明

### 使い方
- gccが必要
```bash
./ビルドした名前 ソースコード.hexl
gcc 生成したアセンブリソースコード -nostdlib
```
- これで実行ファイルが作成可能
* 注意  まだlinuxにしか対応していない windowsの場合はwslを使うことを
おすすめします(それかDocker)

### 予約語
- ret
- cond
- loop
- pub
- const
- struct
- enum
- static
- Self

### プロプロセッサ
- #include
- #asm(..)
    [詳細](./documents/inline_asm.md)
    - (..)に任意のアセンブラの設定ファイルの名前を入れると使える

### 変数の定義方法
1. 普通の変数の定義
2. スタック領域の変数
3. 静的領域の変数
4. 配列の変数
```
d: int = 5
a: [int] = 10
b: ""[int] = 10
c: [int 4] = {1, 2, 3, 4}
```
- [メモリ関連の仕様](./documents/mem.md)
- [変数の扱い(配列も含む)](./documents/variable.md)
- [型について](./documents/type.md)

### 構造体・列挙型
- [構造体・列挙型](./documents/struct_and_enum.md)

### 関数の定義方法
```
name(args: ty): ret_ty {
    ...
    ret value
}
```

### 条件分岐
- [条件分岐について](./documents/match.md)

### インラインアセンブラ
- [アセンブリ言語のフォーマットについて](./documents/asm_fmts.md)

## Changelog
- [CHANGELOG.md](./CHANGELOG.md)

## 📄 ライセンス
このプロジェクトは **[MIT License](./LICENSE.txt)** のもとで公開されています。
個人・商用問わず、自由に使用・改変・配布が可能です。

---
© 2026 Ryuusei/Organization.
