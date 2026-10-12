#!/usr/bin/env bash
set -euo pipefail

if [[ "${PAPER_MESH_DEVCONTAINER:-}" != 1 ]]; then
    echo 'Run this command in the devcontainer: devcontainer exec --workspace-folder . just ci' >&2
    exit 1
fi
