#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)

TARGET_OSS=("linux" "darwin")
TARGET_ARCHES=("arm64" "amd64")

# Create a directory to store the mapped output files
mkdir -p dist

for TARGET_OS in "${TARGET_OSS[@]}"; do
  for TARGET_ARCH in "${TARGET_ARCHES[@]}"; do
    case $TARGET_OS in
    linux) rust_os="linux" ;;
    darwin) rust_os="apple-drawin" ;;
    *) echo "Unsupported OS: $TARGET_OS" && exit 1 ;;
    esac
    case $TARGET_ARCH in
    arm64) rust_arch="aarch64" ;;
    amd64) rust_arch="x86_64" ;;
    esac
    # Find the chatty binary in the <target_arch>*<target_os> directory and the copy it to the dist directory
    # find artifacts -type f -name "*${rust_arch}*${rust_os}*" -exec cp {} "dist/chatty_${TARGET_OS}_${TARGET_ARCH}" \;
    find "${script_dir}/artifacts" -type d -name "*${rust_arch}*${rust_os}*" -exec cp {}/chatty "${script_dir}/dist/chatty_${TARGET_OS}_${TARGET_ARCH}" \;
  done
done

chmod +x dist/chatty_*
