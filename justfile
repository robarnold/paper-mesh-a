set shell := ["bash", "-euo", "pipefail", "-c"]

default:
    @just --list

# Run the same checks locally and on GitHub-hosted runners.
ci: lint toolchain-check firmware-check image

lint:
    bash scripts/require-container.sh
    shellcheck -x scripts/*.sh
    python3 -m json.tool .devcontainer/devcontainer.json > /dev/null
    actionlint
    cd firmware && cargo fmt --check
    git diff --check
    git diff --cached --check

# Check the installed tooling; firmware-build exercises the real linker.
toolchain-check:
    bash scripts/check-toolchain.sh

# Cargo must run here to load firmware/.cargo/config.toml and the esp toolchain.
firmware-check:
    bash scripts/require-container.sh
    cd firmware && cargo clippy --locked --all-targets -- -D warnings

firmware-build:
    bash scripts/require-container.sh
    cd firmware && cargo build --locked --release

# Produce an ELF and merged boot image without accessing hardware.
image: firmware-build
    bash scripts/build-image.sh

fmt:
    bash scripts/require-container.sh
    cd firmware && cargo fmt
