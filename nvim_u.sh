#!/usr/bin/env bash
set -e

echo "==> Removing old Neovim (if installed)..."
sudo apt remove -y neovim || true

echo "==> Installing dependencies..."
sudo apt update
sudo apt install -y curl git xz-utils

TMP_DIR=$(mktemp -d)
cd "$TMP_DIR"

echo "==> Downloading latest Neovim 0.11 release..."
curl -LO https://github.com/neovim/neovim/releases/download/v0.11.4/nvim-linux-x86_64.tar.gz

echo "==> Installing..."
sudo rm -rf /opt/nvim
sudo tar -C /opt -xzf nvim-linux-x86_64.tar.gz
sudo mv /opt/nvim-linux-x86_64 /opt/nvim

echo "==> Creating symlink..."
sudo ln -sf /opt/nvim/bin/nvim /usr/local/bin/nvim

cd
rm -rf "$TMP_DIR"

echo
echo "Installation complete!"
echo

nvim --version
