<p align="center">
  <img src="hexl_lang.png" width="128" />
</p>

# HexlLanguage - 自作プログラミング言語 / Custom Programming Language
Hexlはrustで実装されたプログラミング言語であり
字句解析・構文解析・AST・コンパイラを自前で実装しており、
c言語ライクな言語を作ることを目的としています。
- [X64アセンブラ](https://github.com/ryusei3135/hexl-language/tree/HasmX64)

## 概要（Overview）
c言語ライクな言語

## 特徴（Features）
- 自作プログラミング言語
- 静的型付け
- シンプルな構文

## 目的（Motivation）
- 極限までキーワードを減らしシンプルな構文
- 低レイヤ寄りの言語作成
- c言語ライクな言語の作成

## 説明
### キーワード
- ret
- match
- loop
### プロプロセッサ
- #include
- #asm(..)
### 変数の定義方法
```
name: ty = value
```
### 関数の定義方法
```
name(args: ty): ret_ty {
    ...
    ret value
}
```

## Changelog
- [CHANGELOG.md](./CHANGELOG.md)

## 📄 ライセンス
このプロジェクトは **[MIT License](./LICENSE.txt)** のもとで公開されています。
個人・商用問わず、自由に使用・改変・配布が可能です。

---
© 2026 Ryuusei/Organization.
