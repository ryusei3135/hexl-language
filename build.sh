#!/bin/bash

# エラーが発生したらスクリプトを即座に終了
set -e

# 🔍 Cargoがインストールされているか確認
if ! command -v cargo &> /dev/null; then
    echo "Error: 'cargo' が見つかりません。Rust（Cargo）をインストールしてください。" >&2
    echo "インストールコマンド: curl --proto '=https' --tlsv1.2 -sSf https://rustup.rs | sh" >&2
    exit 1
fi

# プロジェクト名（バイナリ名）を Cargo.toml から自動取得
PROJECT_NAME=$(grep -m 1 '^name =' Cargo.toml | sed -E 's/name = "(.*)"/\1/' | tr -d '\r')


# リリースビルドを実行（デバッグ版にする場合は --release を削除してください）
echo "Building project..."
cargo build --release

# バイナリをルートディレクトリに移動（上書き）
echo "Moving binary to root directory..."
mv "target/release/$PROJECT_NAME" .

echo "✅ Done! '$PROJECT_NAME' has been moved to the root directory."
