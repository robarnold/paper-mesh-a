set shell := ["bash", "-euo", "pipefail", "-c"]

default:
    @just --list

# Run the same checks locally and on GitHub-hosted runners.
ci: lint toolchain-check

lint:
    bash scripts/require-container.sh
    shellcheck -x scripts/*.sh
    python3 -m json.tool .devcontainer/devcontainer.json > /dev/null
    actionlint
    cargo fmt --manifest-path tools/toolchain-smoke/Cargo.toml --check
    git diff --check
    git diff --cached --check

# Compile a no_std library with the real Xtensa compiler, without any hardware.
toolchain-check:
    bash scripts/check-toolchain.sh
