#!/usr/bin/env bash
set -euo pipefail

# Avoid copying image layers into the container when ownership already matches.
owner="$(id -u):$(id -g)"
for directory in /usr/local/cargo /usr/local/rustup /opt/esp; do
    if [[ "$(stat -c '%u:%g' "$directory")" == "$owner" ]]; then
        printf 'Tool ownership already matches %s: %s\n' "$owner" "$directory"
        continue
    fi
    printf 'Restore tool ownership to %s: %s\n' "$owner" "$directory"
    sudo chown -R "$owner" "$directory"
done
