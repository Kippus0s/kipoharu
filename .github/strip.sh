#!/usr/bin/env bash
set -euo pipefail

# Strip symbols from release binaries locally; do not upload debug files.
binary=target/release/kipoharu
case "$RUNNER_OS" in
  macOS)
    strip -S -x target/aarch64-apple-darwin/release/kipoharu
    ;;
  Linux)
    llvm-strip --strip-debug "$binary"
    ;;
  Windows)
    # Debug information is in a separate PDB and is not bundled.
    ;;
esac
