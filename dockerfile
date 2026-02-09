# ベースイメージ
FROM ubuntu:24.04

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    ca-certificates \
    make \
    clang \
    git \
    curl \
    && rm -rf /var/lib/apt/lists/*

RUN apt-get update && \
    apt-get install -y python3 python3-pip && \
    rm -rf /var/lib/apt/lists/*

# Rust インストール
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y

# Rust をパスに追加
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /workspace
CMD ["bash"]
