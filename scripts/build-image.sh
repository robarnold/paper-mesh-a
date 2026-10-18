#!/usr/bin/env bash
set -euo pipefail

# shellcheck source=scripts/require-container.sh
source "$(dirname "${BASH_SOURCE[0]}")/require-container.sh"

artifact_dir=target/artifacts
firmware_elf=firmware/target/xtensa-esp32s3-none-elf/release/paper-mesh-firmware
mkdir -p "$artifact_dir"

# H752-01 / ESP32-S3-WROOM-1: 16 MB flash at 80 MHz.
# Use DIO for the ESP32-S3 ROM bootloader.
# save-image opens no serial port and flashes no hardware.
espflash save-image \
    --skip-update-check \
    --chip esp32s3 \
    --flash-size 16mb \
    --flash-mode dio \
    --flash-freq 80mhz \
    --merge \
    --skip-padding \
    "$firmware_elf" "$artifact_dir/paper-mesh-firmware.bin"

# Keep symbols with the bootloader, partition table, and application image.
cp "$firmware_elf" "$artifact_dir/paper-mesh-firmware.elf"
(cd "$artifact_dir" && sha256sum paper-mesh-firmware.bin paper-mesh-firmware.elf > SHA256SUMS)
