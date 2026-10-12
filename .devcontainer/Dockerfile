FROM rust:1.97.0-bookworm@sha256:8fa55b2f3ddf97471ab6a767bfa3f37e6bad0986ba823e75fea57e2a2a5c3073

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

ARG ESPUP_VERSION=0.17.1
ARG ESP_TOOLCHAIN_VERSION=1.97.0.0
ARG ESPFLASH_VERSION=4.6.0
ARG ESP_GENERATE_VERSION=1.3.0
ARG JUST_VERSION=1.58.0

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        libudev-dev libssl-dev pkg-config python3 python3-yaml shellcheck sudo \
    && rm -rf /var/lib/apt/lists/*

# Rust, Cargo, and espup write only to the container filesystem.
ENV CARGO_HOME=/usr/local/cargo \
    RUSTUP_HOME=/usr/local/rustup \
    ESPUP_EXPORT_FILE=/opt/esp/export.sh \
    PAPER_MESH_DEVCONTAINER=1

RUN useradd --create-home --shell /bin/bash vscode \
    && usermod -aG dialout vscode \
    && echo 'vscode ALL=(root) NOPASSWD:ALL' > /etc/sudoers.d/vscode \
    && chmod 0440 /etc/sudoers.d/vscode \
    && mkdir -p /opt/esp \
    && chown -R vscode:vscode /opt/esp "$CARGO_HOME" "$RUSTUP_HOME"

USER vscode
RUN rustup component add rustfmt clippy \
    && cargo install --locked espup --version "$ESPUP_VERSION" \
    && espup install --targets esp32s3 --toolchain-version "$ESP_TOOLCHAIN_VERSION" \
        --export-file "$ESPUP_EXPORT_FILE"

RUN cargo install --locked espflash --version "$ESPFLASH_VERSION" \
    && cargo install --locked esp-generate --version "$ESP_GENERATE_VERSION" \
    && cargo install --locked just --version "$JUST_VERSION"

# Stable paths also work with non-interactive devcontainer exec commands.
RUN source "$ESPUP_EXPORT_FILE" \
    && esp_gcc="$(command -v xtensa-esp-elf-gcc || command -v xtensa-esp32s3-elf-gcc)" \
    && ln -s "$(dirname "$esp_gcc")" /opt/esp/bin \
    && ln -s "$LIBCLANG_PATH" /opt/esp/libclang
ENV PATH="/opt/esp/bin:/usr/local/cargo/bin:${PATH}" \
    LIBCLANG_PATH=/opt/esp/libclang

COPY --from=rhysd/actionlint:1.7.12@sha256:b1934ee5f1c509618f2508e6eb47ee0d3520686341fec936f3b79331f9315667 /usr/local/bin/actionlint /usr/local/bin/actionlint

WORKDIR /workspaces/paper-mesh-a
