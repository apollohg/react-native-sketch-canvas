#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
CRATE_DIR="$SCRIPT_DIR/sketch-core"
OUT_DIR="$SCRIPT_DIR/bindings"
PKG_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
TARGET_DIR="${CARGO_TARGET_DIR:-$CRATE_DIR/target}"
CDYLIB_PATH="$TARGET_DIR/release/libsketch_core.dylib"

source "$SCRIPT_DIR/symbols.sh"
source "$SCRIPT_DIR/toolchain.sh"

echo "==> Building sketch-core for the host..."
"$RUST_TOOLCHAIN_CARGO" build --manifest-path "$CRATE_DIR/Cargo.toml" --release --features ffi \
    --target-dir "$TARGET_DIR"

echo "==> Verifying the host library's UniFFI surface..."
verify_sketch_symbols host <<< "$(nm -gU "$CDYLIB_PATH" 2>/dev/null || true)"

# uniffi-bindgen's library mode finds Cargo.toml through cargo metadata, so run inside the crate.
cd "$CRATE_DIR"
for language in swift kotlin; do
    echo "==> Generating $language bindings..."
    rm -rf "$OUT_DIR/$language"
    mkdir -p "$OUT_DIR/$language"
    "$RUST_TOOLCHAIN_CARGO" run --release --features cli --target-dir "$TARGET_DIR" --bin uniffi-bindgen -- \
        generate --library "$CDYLIB_PATH" --language "$language" --no-format --out-dir "$OUT_DIR/$language"
done

echo "==> Verifying the generated bindings call every symbol..."
for artifact in \
    "$OUT_DIR/swift/sketch_coreFFI.h" \
    "$OUT_DIR/swift/sketch_core.swift" \
    "$OUT_DIR/kotlin/uniffi/sketch_core/sketch_core.kt"; do
    verify_sketch_symbols "$(basename "$artifact")" < "$artifact"
done

echo "==> Copying the Swift binding into ios/ for CocoaPods..."
cp "$OUT_DIR/swift/sketch_core.swift" "$PKG_DIR/ios/Generated_sketch_core.swift"
mkdir -p "$PKG_DIR/ios/sketch_coreFFI"
cp "$OUT_DIR/swift/sketch_coreFFI.h" "$PKG_DIR/ios/sketch_coreFFI/sketch_coreFFI.h"
cp "$OUT_DIR/swift/sketch_coreFFI.modulemap" "$PKG_DIR/ios/sketch_coreFFI/module.modulemap"

echo "==> Bindings generated: $OUT_DIR"
