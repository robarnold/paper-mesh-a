set shell := ["bash", "-euo", "pipefail", "-c"]

default:
    @just --list

_require-container:
    bash scripts/require-container.sh

# Run the same checks locally and on GitHub-hosted runners.
ci: lint toolchain-check native-check firmware-check firmware-demo-check image simulator-snapshot

lint: _require-container
    shellcheck -x scripts/*.sh
    python3 -m json.tool .devcontainer/devcontainer.json > /dev/null
    actionlint
    cargo fmt --all --check
    cd firmware && cargo fmt --check
    git diff --check
    git diff --cached --check

# Check the installed tools. The firmware-build recipe uses the real linker.
toolchain-check: _require-container
    bash scripts/check-toolchain.sh

native-check: _require-container
    python3 -B -m unittest discover -s scripts/tests
    cargo clippy --locked --workspace --all-targets --keep-going -- -D warnings
    cargo nextest run --locked --workspace
    cargo test --locked --workspace --doc

# Rust runs in the container. The browser displays frames and sends input.
sim: _require-container
    cargo run --locked -p paper-mesh-simulator

# Rebuild on edits and refresh open UI guide tabs.
sim-watch: _require-container
    python3 scripts/watch_simulator.py

simulator-snapshot: _require-container
    cargo run --locked -p paper-mesh-simulator -- --snapshot target/simulator/hello.png
    cargo run --locked -p paper-mesh-simulator -- --snapshot-native target/simulator/native.png

# Cargo must run here to load firmware/.cargo/config.toml and the esp toolchain.
firmware-check: _require-container
    cd firmware && cargo clippy --locked --all-targets --all-features -- -D warnings
    cd firmware && cargo clippy --locked -p paper-mesh-esp-adapters --lib -- -D warnings

firmware-demo-check: _require-container
    cd firmware && cargo build --locked --release --features radio-scan-demo

firmware-build: _require-container
    cd firmware && cargo build --locked --release

# Produce an ELF and merged boot image without hardware access.
image: firmware-build
    bash scripts/build-image.sh

fmt: _require-container
    cargo fmt --all
    cd firmware && cargo fmt
