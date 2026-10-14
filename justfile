set shell := ["bash", "-euo", "pipefail", "-c"]

default:
    @just --list

# Run the same checks locally and on GitHub-hosted runners.
ci: lint toolchain-check native-check firmware-check image simulator-snapshot

lint:
    bash scripts/require-container.sh
    shellcheck -x scripts/*.sh
    python3 -m json.tool .devcontainer/devcontainer.json > /dev/null
    actionlint
    cargo fmt --all --check
    cd firmware && cargo fmt --check
    git diff --check
    git diff --cached --check

# Check the installed tools. The firmware-build recipe uses the real linker.
toolchain-check:
    bash scripts/check-toolchain.sh

native-check:
    bash scripts/require-container.sh
    cargo clippy --locked --workspace --all-targets -- -D warnings
    cargo test --locked --workspace

# Rust runs in the container. The browser displays frames and sends input.
sim:
    bash scripts/require-container.sh
    cargo run --locked -p paper-mesh-simulator

simulator-snapshot:
    bash scripts/require-container.sh
    cargo run --locked -p paper-mesh-simulator -- --snapshot target/simulator/hello.png
    cargo run --locked -p paper-mesh-simulator -- --snapshot-native target/simulator/native.png

# Cargo must run here to load firmware/.cargo/config.toml and the esp toolchain.
firmware-check:
    bash scripts/require-container.sh
    cd firmware && cargo clippy --locked --all-targets -- -D warnings

firmware-build:
    bash scripts/require-container.sh
    cd firmware && cargo build --locked --release

# Produce an ELF and merged boot image without hardware access.
image: firmware-build
    bash scripts/build-image.sh

fmt:
    bash scripts/require-container.sh
    cargo fmt --all
    cd firmware && cargo fmt
