# ベースイメージ
FROM ubuntu:24.04

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    ca-certificates \
    make \
    gcc \
    git \
    curl \
    && rm -rf /var/lib/apt/lists/*

# # rustupを使ってインストール
# RUN curl https://sh.rustup.rs -sSf | sh -s -- -y
# ENV PATH="/root/.cargo/bin:${PATH}"

# # バージョン確認（デバッグ用）
# RUN rustc --version && cargo --version

WORKDIR /workspace
CMD ["bash"]
