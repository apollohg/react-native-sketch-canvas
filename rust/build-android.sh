#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
CRATE_DIR="$SCRIPT_DIR/sketch-core"
OUT_DIR="$SCRIPT_DIR/android"
LIB_NAME="libsketch_core.so"
MIN_SDK_VERSION=24

source "$SCRIPT_DIR/symbols.sh"
source "$SCRIPT_DIR/toolchain.sh"
source "$SCRIPT_DIR/release-profile.sh"
export CARGO="$RUST_TOOLCHAIN_CARGO"
TARGET_DIR="${CARGO_TARGET_DIR:-$CRATE_DIR/target}"
if [[ -n "${RUST_TOOLCHAIN_CARGO_PLUGIN_DIR:-}" ]]; then
    export PATH="$RUST_TOOLCHAIN_CARGO_PLUGIN_DIR:$PATH"
fi
if [[ -n "${CARGO_HOME:-}" ]]; then
    export PATH="$CARGO_HOME/bin:$PATH"
fi

if ! command -v cargo-ndk &>/dev/null; then
    echo "ERROR: cargo-ndk is not installed. Install it with: cargo install cargo-ndk" >&2
    exit 1
fi

# Bash 3.2 on macOS has no associative arrays, so keep "target abi" pairs.
TARGET_ABI_PAIRS=(
    "aarch64-linux-android arm64-v8a"
    "armv7-linux-androideabi armeabi-v7a"
    "i686-linux-android x86"
    "x86_64-linux-android x86_64"
)

echo "==> Building sketch-core for Android targets..."
cd "$CRATE_DIR"
for pair in "${TARGET_ABI_PAIRS[@]}"; do
    target="${pair%% *}"
    abi="${pair#* }"
    echo "  -> $target ($abi)"
    "$RUST_TOOLCHAIN_CARGO" ndk --target "$target" --platform "$MIN_SDK_VERSION" \
        build --release --features ffi --target-dir "$TARGET_DIR"
    mkdir -p "$OUT_DIR/$abi"
    bash "$SCRIPT_DIR/strip-native-library.sh" android \
        "$TARGET_DIR/$target/release/$LIB_NAME" "$OUT_DIR/$abi/$LIB_NAME"
    test -s "$OUT_DIR/$abi/$LIB_NAME"
    verify_sketch_symbols "$abi" <<< "$(nm -D -g "$OUT_DIR/$abi/$LIB_NAME" 2>/dev/null || true)"
done

echo "==> Android build complete: $OUT_DIR/"
