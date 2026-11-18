#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
CRATE_DIR="$SCRIPT_DIR/sketch-core"
OUT_DIR="$SCRIPT_DIR/ios"
PKG_IOS_DIR="$(cd "$SCRIPT_DIR/.." && pwd)/ios"
LIB_NAME="libsketch_core.a"
IOS_DEPLOYMENT_TARGET="16.4"

source "$SCRIPT_DIR/symbols.sh"
source "$SCRIPT_DIR/toolchain.sh"
source "$SCRIPT_DIR/release-profile.sh"
TARGET_DIR="${CARGO_TARGET_DIR:-$CRATE_DIR/target}"

IOS_TARGETS=(
    "aarch64-apple-ios"
    "aarch64-apple-ios-sim"
    "x86_64-apple-ios"
)

echo "==> Building sketch-core for iOS targets..."
for target in "${IOS_TARGETS[@]}"; do
    echo "  -> $target"
    IPHONEOS_DEPLOYMENT_TARGET="$IOS_DEPLOYMENT_TARGET" \
        "$RUST_TOOLCHAIN_CARGO" rustc --manifest-path "$CRATE_DIR/Cargo.toml" --release --features ffi \
        --target "$target" --target-dir "$TARGET_DIR" --crate-type staticlib
    mkdir -p "$OUT_DIR/$target"
    bash "$SCRIPT_DIR/strip-native-library.sh" ios \
        "$TARGET_DIR/$target/release/$LIB_NAME" "$OUT_DIR/$target/$LIB_NAME"
done

echo "==> Verifying the UniFFI surface in every slice..."
for target in "${IOS_TARGETS[@]}"; do
    verify_sketch_symbols "$target" <<< "$(nm -gU "$OUT_DIR/$target/$LIB_NAME" 2>/dev/null || true)"
done

echo "==> Creating the simulator fat library..."
mkdir -p "$OUT_DIR/sim-fat"
lipo -create \
    "$OUT_DIR/aarch64-apple-ios-sim/$LIB_NAME" \
    "$OUT_DIR/x86_64-apple-ios/$LIB_NAME" \
    -output "$OUT_DIR/sim-fat/$LIB_NAME"

echo "==> Creating SketchCore.xcframework..."
rm -rf "$OUT_DIR/SketchCore.xcframework"
xcodebuild -create-xcframework \
    -library "$OUT_DIR/aarch64-apple-ios/$LIB_NAME" \
    -library "$OUT_DIR/sim-fat/$LIB_NAME" \
    -output "$OUT_DIR/SketchCore.xcframework"

echo "==> Syncing the XCFramework into ios/ for CocoaPods..."
rm -rf "$PKG_IOS_DIR/SketchCore.xcframework"
cp -R "$OUT_DIR/SketchCore.xcframework" "$PKG_IOS_DIR/SketchCore.xcframework"

echo "==> iOS build complete: $PKG_IOS_DIR/SketchCore.xcframework"
