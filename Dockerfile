# syntax=docker/dockerfile:1.7

# ===========================================================================
# Stage: test — lightweight image for fmt + clippy + coverage + audit
# ===========================================================================
FROM rust:1.93.1-slim-bookworm AS base

ARG EXTRA_CA_CERTS=""

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        curl ca-certificates && \
    rm -rf /var/lib/apt/lists/*

RUN if [ -n "$EXTRA_CA_CERTS" ]; then printf '%s\n' "$EXTRA_CA_CERTS" >> /etc/ssl/certs/ca-certificates.crt; fi

WORKDIR /app

FROM base AS test

RUN rustup component add rustfmt clippy llvm-tools-preview

RUN curl -LsSf https://github.com/cargo-bins/cargo-binstall/releases/latest/download/cargo-binstall-x86_64-unknown-linux-musl.tgz \
        | tar xzf - -C /usr/local/cargo/bin && \
    cargo binstall -y cargo-llvm-cov

RUN cargo install cargo-audit --locked

ENV XSFX_SKIP_STUB_BUILD=1

COPY Cargo.toml Cargo.lock ./
COPY build.rs ./
COPY src/ src/
COPY tests/ tests/

CMD cargo fmt --all -- --check \
    && cargo clippy --lib --test integration -- -D warnings \
    && cargo llvm-cov --lib --test integration \
        --ignore-filename-regex 'bin/' \
        --fail-under-lines 100 --fail-under-functions 100 \
    && cargo audit

# ===========================================================================
# Stage: cross — heavyweight image with ALL cross-compilation toolchains
# Supports: Linux gnu+musl, macOS via osxcross, Windows via mingw+xwin
# ===========================================================================
FROM rust:1.93.1-bookworm AS cross

ARG MAC_SDK_URL=https://github.com/joseluisq/macosx-sdks/releases/download/15.2/MacOSX15.2.sdk.tar.xz

SHELL ["/bin/bash", "-lc"]

ENV DEBIAN_FRONTEND=noninteractive \
    RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    MACOSX_DEPLOYMENT_TARGET=11.0 \
    RUSTUP_USE_CURL=1 \
    SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt \
    CARGO_HTTP_CAINFO=/etc/ssl/certs/ca-certificates.crt

RUN set -euo pipefail \
    && apt-get update \
    && apt-get install -y --no-install-recommends \
         ca-certificates curl git xz-utils cpio \
         build-essential pkg-config \
         clang lld make cmake gnupg \
         gcc-aarch64-linux-gnu \
         libc6-dev-arm64-cross \
         crossbuild-essential-arm64 \
         mingw-w64 \
         musl-tools \
         file \
    && update-ca-certificates \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/* \
    && echo 'export PATH=/usr/local/cargo/bin:$PATH' > /etc/profile.d/cargo.sh

ARG EXTRA_CA_CERTS=""
RUN if [ -n "$EXTRA_CA_CERTS" ]; then printf '%s\n' "$EXTRA_CA_CERTS" >> /etc/ssl/certs/ca-certificates.crt; fi

# Zig for aarch64 musl cross-compilation (replaces flaky musl.cc toolchain)
RUN set -euo pipefail \
    && ZIG_VERSION=0.15.2 \
    && curl -sSfL "https://ziglang.org/download/${ZIG_VERSION}/zig-x86_64-linux-${ZIG_VERSION}.tar.xz" \
        | tar xJf - -C /opt \
    && ln -s "/opt/zig-x86_64-linux-${ZIG_VERSION}/zig" /usr/local/bin/zig \
    && printf '#!/bin/sh\nexec zig cc -target aarch64-linux-musl "$@"\n' > /usr/local/bin/aarch64-linux-musl-gcc \
    && printf '#!/bin/sh\nexec zig ar "$@"\n' > /usr/local/bin/aarch64-linux-musl-ar \
    && printf '#!/bin/sh\nexec zig ranlib "$@"\n' > /usr/local/bin/aarch64-linux-musl-ranlib \
    && chmod +x /usr/local/bin/aarch64-linux-musl-gcc \
                /usr/local/bin/aarch64-linux-musl-ar \
                /usr/local/bin/aarch64-linux-musl-ranlib

# Stable Rust targets
RUN set -euo pipefail \
    && . /etc/profile \
    && rustup target add \
         x86_64-unknown-linux-gnu \
         aarch64-unknown-linux-gnu \
         x86_64-unknown-linux-musl \
         aarch64-unknown-linux-musl \
         x86_64-apple-darwin \
         aarch64-apple-darwin \
         x86_64-pc-windows-gnu \
         x86_64-pc-windows-msvc \
         aarch64-pc-windows-msvc

# Nightly Rust for stub minification (build-std + panic=immediate-abort)
RUN set -euo pipefail \
    && . /etc/profile \
    && rustup install nightly \
    && rustup component add rust-src --toolchain nightly \
    && rustup target add --toolchain nightly \
         x86_64-unknown-linux-gnu \
         aarch64-unknown-linux-gnu \
         x86_64-unknown-linux-musl \
         aarch64-unknown-linux-musl \
         x86_64-apple-darwin \
         aarch64-apple-darwin \
         x86_64-pc-windows-gnu \
         x86_64-pc-windows-msvc \
         aarch64-pc-windows-msvc

# xwin for MSVC cross-compilation
RUN set -euo pipefail \
    && . /etc/profile \
    && cargo install xwin --version 0.8.0 \
    && xwin --accept-license --arch x86_64,aarch64 splat --output /opt/xwin \
    && rm -rf /usr/local/cargo/registry/cache

# osxcross for macOS cross-compilation
RUN set -euo pipefail \
    && git config --global http.sslverify false \
    && cd /opt && git clone --depth=1 https://github.com/tpoechtrager/osxcross.git \
    && cd osxcross && mkdir -p tarballs \
    && SDK_FILE="tarballs/$(basename "$MAC_SDK_URL")" \
    && for i in 1 2 3; do curl -sSfLk "$MAC_SDK_URL" -o "$SDK_FILE" && break; echo "retry $i"; sleep 5; done \
    && test -s "$SDK_FILE" || { echo "SDK download failed" >&2; exit 1; } \
    && SDK_BASENAME=$(basename "$SDK_FILE") \
    && SDK_VERSION=$(echo "$SDK_BASENAME" | sed -E 's/^MacOSX([0-9]+(\.[0-9]+)*).*$/\1/') \
    && UNATTENDED=1 SDK_VERSION="$SDK_VERSION" CC=clang CXX=clang++ JOBS=$(nproc) ./build.sh \
    && rm -rf /opt/osxcross/tarballs /opt/osxcross/.git /opt/osxcross/build \
    && find /opt/osxcross -name "*.o" -delete \
    && find /opt/osxcross -name "*.a" -delete 2>/dev/null || true

# UPX for post-build stub compression
RUN set -euo pipefail \
    && UPX_VERSION=5.0.1 \
    && curl -sSfL "https://github.com/upx/upx/releases/download/v${UPX_VERSION}/upx-${UPX_VERSION}-amd64_linux.tar.xz" \
        | tar xJf - -C /usr/local/bin --strip-components=1 "upx-${UPX_VERSION}-amd64_linux/upx"

# xstrip for ELF dead-code removal (musl stubs where UPX cannot be used)
RUN set -euo pipefail \
    && XSTRIP_VERSION=0.1.0 \
    && curl -sSfL "https://github.com/ratushnyi-labs/xstrip/releases/download/v${XSTRIP_VERSION}/xstrip-linux-amd64.tar.gz" \
        | tar xzf - -C /usr/local/bin

# Cargo configuration for cross-linkers
RUN mkdir -p /usr/local/cargo && \
    echo '[http]' > /usr/local/cargo/config.toml && \
    echo 'cainfo = "/etc/ssl/certs/ca-certificates.crt"' >> /usr/local/cargo/config.toml && \
    echo '' >> /usr/local/cargo/config.toml && \
    echo '[target.x86_64-pc-windows-msvc]' >> /usr/local/cargo/config.toml && \
    echo 'linker = "lld-link"' >> /usr/local/cargo/config.toml && \
    echo 'rustflags = ["-Lnative=/opt/xwin/crt/lib/x86_64", "-Lnative=/opt/xwin/sdk/lib/um/x86_64", "-Lnative=/opt/xwin/sdk/lib/ucrt/x86_64"]' >> /usr/local/cargo/config.toml && \
    echo '' >> /usr/local/cargo/config.toml && \
    echo '[target.aarch64-pc-windows-msvc]' >> /usr/local/cargo/config.toml && \
    echo 'linker = "lld-link"' >> /usr/local/cargo/config.toml && \
    echo 'rustflags = ["-Lnative=/opt/xwin/crt/lib/aarch64", "-Lnative=/opt/xwin/sdk/lib/um/aarch64", "-Lnative=/opt/xwin/sdk/lib/ucrt/aarch64"]' >> /usr/local/cargo/config.toml && \
    echo '' >> /usr/local/cargo/config.toml && \
    echo '[target.x86_64-pc-windows-gnu]' >> /usr/local/cargo/config.toml && \
    echo 'rustflags = ["-C", "link-arg=-Wl,--exclude-libs=msvcrt.lib"]' >> /usr/local/cargo/config.toml

ENV PATH=/opt/osxcross/target/bin:/usr/local/cargo/bin:$PATH \
    CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc \
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
    CC_x86_64_unknown_linux_musl=musl-gcc \
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=musl-gcc \
    CC_aarch64_unknown_linux_musl=aarch64-linux-musl-gcc \
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=aarch64-linux-musl-gcc \
    CC_x86_64_pc_windows_gnu=x86_64-w64-mingw32-gcc \
    CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc \
    CC_x86_64_pc_windows_msvc=clang-cl \
    CXX_x86_64_pc_windows_msvc=clang-cl \
    CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER=lld-link \
    CC_aarch64_pc_windows_msvc=clang-cl \
    CXX_aarch64_pc_windows_msvc=clang-cl \
    CARGO_TARGET_AARCH64_PC_WINDOWS_MSVC_LINKER=lld-link \
    CC_x86_64_apple_darwin=o64-clang \
    CC_aarch64_apple_darwin=oa64-clang \
    CXX_x86_64_apple_darwin=o64-clang++ \
    CXX_aarch64_apple_darwin=oa64-clang++ \
    CARGO_TARGET_X86_64_APPLE_DARWIN_LINKER=o64-clang \
    CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER=oa64-clang

VOLUME ["/usr/local/cargo/registry"]
WORKDIR /project

COPY scripts/xsfx-entrypoint.sh /usr/local/bin/xsfx-entrypoint.sh
RUN chmod +x /usr/local/bin/xsfx-entrypoint.sh

ENTRYPOINT ["/usr/local/bin/xsfx-entrypoint.sh"]
