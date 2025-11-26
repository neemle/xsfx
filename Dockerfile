# syntax=docker/dockerfile:1.7

# Builder image that contains all cross-compilation dependencies to build xsfx
# for Linux (gnu+musl), macOS (via osxcross), and Windows (x86_64-gnu).
#
# Build once and reuse for all builds:
#   docker build --platform linux/amd64 -t xsfx-build \
#     --build-arg MAC_SDK_URL=https://github.com/alexey-lysiuk/macos-sdk/releases/download/15.5/MacOSX15.5.tar.xz \
#     .
#
# Then run builds with:
#   ./build.sh   (auto-uses this image)

ARG RUST_IMAGE=rust:1.83-bullseye
FROM ${RUST_IMAGE} as base

ARG MAC_SDK_URL=https://github.com/alexey-lysiuk/macos-sdk/releases/download/15.5/MacOSX15.5.tar.xz

## Use bash for RUN commands so `set -euo pipefail` is supported
SHELL ["/bin/bash", "-lc"]

ENV DEBIAN_FRONTEND=noninteractive \
    RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    MACOSX_DEPLOYMENT_TARGET=11.0

RUN set -euo pipefail \
    && echo "Installing system packages" \
    && apt-get update \
    && apt-get install -y --no-install-recommends \
         ca-certificates curl git xz-utils cpio \
         build-essential pkg-config \
         clang lld make cmake gnupg \
         liblzma-dev \
         gcc-aarch64-linux-gnu \
         libc6-dev-arm64-cross \
         crossbuild-essential-arm64 \
         mingw-w64 \
         musl-tools \
         file \
    && update-ca-certificates \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/* /var/cache/apt/archives/* /tmp/* /var/tmp/*



# Install aarch64 musl cross from musl.cc (not in Debian repos)
RUN set -euo pipefail \
    && if ! command -v aarch64-linux-musl-gcc >/dev/null 2>&1; then \
         echo "Installing aarch64-linux-musl cross toolchain..."; \
         curl -sSfLk https://musl.cc/aarch64-linux-musl-cross.tgz -o /tmp/aarch64-linux-musl-cross.tgz; \
         tar -C /opt -xf /tmp/aarch64-linux-musl-cross.tgz; \
         echo 'export PATH=/opt/aarch64-linux-musl-cross/bin:$PATH' >> /etc/profile.d/muslcc.sh; \
         rm -f /tmp/aarch64-linux-musl-cross.tgz; \
       fi

# Ensure rustup and required targets
RUN set -euo pipefail \
    && if ! command -v rustup >/dev/null 2>&1; then \
         echo "Installing rustup..."; \
         curl -sSfL https://sh.rustup.rs | sh -s -- -y --profile minimal; \
       fi \
    && . /etc/profile \
    && export PATH="/usr/local/cargo/bin:$PATH" \
    && rustup target add x86_64-unknown-linux-gnu \
    && rustup target add aarch64-unknown-linux-gnu \
    && rustup target add x86_64-unknown-linux-musl \
    && rustup target add aarch64-unknown-linux-musl \
    && rustup target add x86_64-apple-darwin \
    && rustup target add aarch64-apple-darwin \
    && rustup target add x86_64-pc-windows-gnu \
    && rustup target add x86_64-pc-windows-msvc \
    && rustup target add aarch64-pc-windows-msvc \
    && rm -rf /usr/local/cargo/registry/cache

# Install xwin for MSVC cross-compilation and setup Windows SDK
RUN set -euo pipefail \
    && . /etc/profile \
    && export PATH="/usr/local/cargo/bin:$PATH" \
    && cargo install xwin --version 0.6.0 \
    && xwin --accept-license splat --output /opt/xwin \
    && rm -rf /usr/local/cargo/registry/cache

# Build and install osxcross with the provided SDK
RUN set -euo pipefail \
    && git config --global http.sslverify false \
    && mkdir -p /opt && cd /opt \
    && git clone --depth=1 https://github.com/tpoechtrager/osxcross.git \
    && cd osxcross \
    && mkdir -p tarballs \
    && echo "Downloading macOS SDK: ${MAC_SDK_URL}" \
    && SDK_FILE="tarballs/$(basename "$MAC_SDK_URL")" \
    && for i in 1 2 3; do curl -sSfLk "$MAC_SDK_URL" -o "$SDK_FILE" && break; echo "retry $i" && sleep 5; done \
    && if [ ! -s "$SDK_FILE" ]; then echo "Failed to download SDK" >&2; exit 1; fi \
    && if [ "${SDK_FILE##*.sdk.tar.xz}" = "$SDK_FILE" ]; then \
         SDK_SDK_NAME="${SDK_FILE%.tar.xz}.sdk.tar.xz"; \
         cp -f "$SDK_FILE" "$SDK_SDK_NAME"; \
         rm -f "$SDK_FILE"; \
         SDK_FILE="$SDK_SDK_NAME"; \
       fi \
    && SDK_BASENAME=$(basename "$SDK_FILE") \
    && SDK_VERSION=$(echo "$SDK_BASENAME" | sed -E 's/^MacOSX([0-9]+(\.[0-9]+)*).*$/\1/') \
    && echo "Using SDK_VERSION=$SDK_VERSION" \
    && UNATTENDED=1 SDK_VERSION="$SDK_VERSION" CC=clang CXX=clang++ ./tools/gen_sdk_package_pbzx.sh tarballs || true \
    && UNATTENDED=1 SDK_VERSION="$SDK_VERSION" CC=clang CXX=clang++ JOBS=$(nproc) ./build.sh \
    && rm -rf /opt/osxcross/tarballs /opt/osxcross/.git /opt/osxcross/build \
    && rm -rf /tmp/* /var/tmp/* \
    && find /opt/osxcross -name "*.o" -delete \
    && find /opt/osxcross -name "*.a" -delete 2>/dev/null || true

# Configure cargo to use custom CA certificate
RUN mkdir -p /usr/local/cargo && \
    echo '[http]' > /usr/local/cargo/config.toml && \
    echo 'cainfo = "/etc/ssl/certs/ca-certificates.crt"' >> /usr/local/cargo/config.toml

# Configure environment for cross linkers and osxcross
ENV PATH=/opt/aarch64-linux-musl-cross/bin:/opt/osxcross/target/bin:/usr/local/cargo/bin:$PATH \
    SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt \
    CARGO_HTTP_CAINFO=/etc/ssl/certs/ca-certificates.crt \
    XWIN_ARCH=x86_64,aarch64 \
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

# Configure cargo for MSVC targets with xwin using lld-link
RUN echo '' >> /usr/local/cargo/config.toml && \
    echo '[target.x86_64-pc-windows-msvc]' >> /usr/local/cargo/config.toml && \
    echo 'linker = "lld-link"' >> /usr/local/cargo/config.toml && \
    echo 'rustflags = ["-Lnative=/opt/xwin/crt/lib/x86_64", "-Lnative=/opt/xwin/sdk/lib/um/x86_64", "-Lnative=/opt/xwin/sdk/lib/ucrt/x86_64"]' >> /usr/local/cargo/config.toml && \
    echo '' >> /usr/local/cargo/config.toml && \
    echo '[target.aarch64-pc-windows-msvc]' >> /usr/local/cargo/config.toml && \
    echo 'linker = "lld-link"' >> /usr/local/cargo/config.toml && \
    echo 'rustflags = ["-Lnative=/opt/xwin/crt/lib/arm64", "-Lnative=/opt/xwin/sdk/lib/um/arm64", "-Lnative=/opt/xwin/sdk/lib/ucrt/arm64"]' >> /usr/local/cargo/config.toml && \
    echo '' >> /usr/local/cargo/config.toml && \
    echo '[target.x86_64-pc-windows-gnu]' >> /usr/local/cargo/config.toml && \
    echo 'rustflags = ["-C", "link-arg=-Wl,--exclude-libs=msvcrt.lib"]' >> /usr/local/cargo/config.toml

# Create a cargo registry/cache directory to be mounted as a volume optionally
VOLUME ["/usr/local/cargo/registry"]

WORKDIR /project

# Copy entrypoint script that performs the full build inside the container
COPY scripts/xsfx-entrypoint.sh /usr/local/bin/xsfx-entrypoint.sh
RUN chmod +x /usr/local/bin/xsfx-entrypoint.sh

# Run the entrypoint by default; build.sh simply runs the container
ENTRYPOINT ["/usr/local/bin/xsfx-entrypoint.sh"]
