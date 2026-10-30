# hexl_lsp

hexl 用の LSP サーバー(stdio)と VS Code 拡張。

`hexl_frontend` は `crate::compiler::*` に依存していて単体ではビルドできないため、
キーワード表(`lex/lexer.rs`)と構文の仕様(`#include` の書式など)に合わせた
独自の軽量字句解析(`src/scan.rs`)で動きます。

## 機能
| 機能 | 内容 |
|------|------|
| 診断 | 括弧の不一致/閉じ忘れ、閉じていない文字列・`/* */`、`loop` の外の `break`/`continue`、`#if` 外での重複定義、`#include` 先が見つからない |
| 補完 | 予約語、スニペット(`struct` `impl` `loop` `cond` `fn` ...)、定義、モジュール名、ファイル内の名前 |
| | `mod::` → モジュールの公開要素 / `Type::` → メゾット・バリアント、`self.` `self->` → 自身の型のメンバー、`x.` → 見えているメンバー |
| | `#` → 命令とマクロ、`#include "` → ファイルパス |
| ホバー | 予約語・`#命令`の説明、シグネチャ、直前の `//` コメント、`#include` のパス解決結果 |
| 定義へ移動 | `#include` をまたいで解決。`mod::name` / `self->name` / `#MACRO` / `#include` のパス |
| 参照・ハイライト | ワークスペースの全 `.hexl` が対象 |
| リネーム | 関数・型・メンバー・マクロ(ローカル変数は対象外)。ファイルをまたいで変更 |
| シグネチャヘルプ | `f(` `,` で引数を表示(ジェネリクス `f<T>(` 対応、`x.m(` は `self` を除く) |
| シンボル | ドキュメント(struct/variant/impl の下にメゾット・メンバーを階層表示)、ワークスペース |
| 折りたたみ | `{ }`、`/* */`、`#if`〜`#endif` |
| セマンティックトークン | 予約語・型・関数・メゾット・メンバー・マクロ・文字列・数値・モジュール名 |

## `#include` の解決
書式は `lex/PREPROCESSOR.md` と `parse/preproc.rs` に合わせています
(`"a.hexl"` / `Name="a.hexl"` / `"a.hexl"::func` / `"a.hexl"::*` / `std::"os/vec.hexl"` / 旧形式 `a::b`)。
- 通常のパス: 本来は「コンパイルを始めたファイルのディレクトリ」基準ですが、LSP からは起点が
  分からないため、**開いているファイルのディレクトリ → その親(8階層まで) → ワークスペースのルート** の順に探します。
- 標準ライブラリ(`std::"..."`): `initializationOptions.libraryPath` → 環境変数 `HEXL_LIBRARY` →
  ワークスペースの `library/` → 親ディレクトリの `library/` の順に探します。

## ビルド・テスト
    cargo build --release        # target/release/hexl_lsp
    cargo test                   # 単体テスト
    python3 tests/e2e.py         # E2E(examples/project を使用)

## VS Code
    cd vscode-hexl && npm install
    # F5 で拡張を起動するか、`npx @vscode/vsce package` で .vsix を作成
設定: `hexl.server.path`(hexl_lsp のパス)、`hexl.library.path`(標準ライブラリ)。

## Neovim
    vim.filetype.add({ extension = { hexl = "hexl" } })
    vim.api.nvim_create_autocmd("FileType", { pattern = "hexl", callback = function()
      vim.lsp.start({
        name = "hexl_lsp", cmd = { "/path/to/hexl_lsp" },
        root_dir = vim.fs.root(0, { "library", ".git" }),
        init_options = { libraryPath = "/path/to/hexl/library" },
      })
    end })

## 制限(字句レベルの解析であることによるもの)
- 型検査、未定義の名前、型に基づくメンバー補完(`x.` は見えている全メンバーを出す)は未対応。
- パーサーのエラー(構文エラーの詳細)は出ません。出せるのは括弧・文字列・コメント・`break`/`continue` など字句で分かるものだけ。
- `#if` の無効な枝は評価しないので、枝ごとに括弧の数が違うコードでは括弧の診断が誤ることがあります。
- `hexl_frontend` をビルドできる環境があれば、`scan.rs` を `lex::Lexer` / `parse::Parser` の結果に
  置き換えると、診断と補完の精度を上げられます。
