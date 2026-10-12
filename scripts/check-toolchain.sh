#!/usr/bin/env bash
set -euo pipefail

# shellcheck source=scripts/require-container.sh
source "$(dirname "${BASH_SOURCE[0]}")/require-container.sh"

rustc --version
cargo --version
rustc +esp --version
espup --version
espflash --version
esp-generate --version
just --version
esp_gcc="$(command -v xtensa-esp-elf-gcc || command -v xtensa-esp32s3-elf-gcc)"
"$esp_gcc" --version
test -d "$LIBCLANG_PATH"

cargo +esp build \
    --manifest-path tools/toolchain-smoke/Cargo.toml \
    --locked \
    --target xtensa-esp32s3-none-elf \
    --release \
    -Zbuild-std=core
