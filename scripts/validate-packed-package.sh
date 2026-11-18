#!/usr/bin/env bash
#
# Checks an `npm pack` tarball: every native artefact a consumer needs is present and exports the
# expected UniFFI surface, and no build output, test or fixture directory leaks in.
#
# Usage: scripts/validate-packed-package.sh PACKAGE.tgz

set -euo pipefail

[[ $# -eq 1 && -f "$1" ]] || { echo "usage: $0 PACKAGE.tgz" >&2; exit 2; }
tarball="$1"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$repo_root/rust/symbols.sh"

ANDROID_ABIS=(arm64-v8a armeabi-v7a x86 x86_64)
IOS_SLICES=(ios-arm64 ios-arm64_x86_64-simulator)

REQUIRED=(
    package/package.json
    package/README.md
    package/LICENSE
    package/FORMAT.md
    package/THIRD_PARTY_NOTICES.md
    package/RUST-STANDARD-LIBRARY-NOTICES.html
    package/dist/index.js
    package/dist/index.d.ts
    package/src/index.ts
    package/expo-module.config.json
    package/ReactNativeSketchCanvas.podspec
    package/ios/Generated_sketch_core.swift
    package/ios/SketchCanvasModule.swift
    package/ios/sketch_coreFFI/sketch_coreFFI.h
    package/ios/sketch_coreFFI/module.modulemap
    package/ios/SketchCore.xcframework/Info.plist
    package/android/build.gradle
    package/android/consumer-rules.pro
    package/android/src/main/java/com/apollohg/sketchcanvas/SketchCanvasModule.kt
    package/rust/bindings/kotlin/uniffi/sketch_core/sketch_core.kt
)
for slice in "${IOS_SLICES[@]}"; do
    REQUIRED+=("package/ios/SketchCore.xcframework/$slice/libsketch_core.a")
done
for abi in "${ANDROID_ABIS[@]}"; do
    REQUIRED+=("package/rust/android/$abi/libsketch_core.so")
done

FORBIDDEN_PATTERN='(/target/|__tests__|/src/test/|/src/androidTest/|^package/ios-tests/|^package/example/|^package/fixtures/|^package/rust/sketch-core/|\.superpowers|node_modules|/build/)'

contents="$(tar -tzf "$tarball")"
failures=0

for path in "${REQUIRED[@]}"; do
    if ! grep -qxF "$path" <<< "$contents"; then
        echo "error: missing $path" >&2
        failures=$((failures + 1))
    fi
done

forbidden="$(grep -E "$FORBIDDEN_PATTERN" <<< "$contents" || true)"
if [[ -n "$forbidden" ]]; then
    echo "error: the package contains build, test or fixture files:" >&2
    echo "$forbidden" >&2
    failures=$((failures + 1))
fi

if [[ $failures -eq 0 ]]; then
    extract_dir="$(mktemp -d)"
    trap 'rm -rf "$extract_dir"' EXIT
    tar -xzf "$tarball" -C "$extract_dir"
    echo "==> Verifying the UniFFI surface of every packaged library..."
    for slice in "${IOS_SLICES[@]}"; do
        verify_sketch_symbols "$slice" <<< "$(nm -gU "$extract_dir/package/ios/SketchCore.xcframework/$slice/libsketch_core.a" 2>/dev/null || true)" \
            || failures=$((failures + 1))
    done
    for abi in "${ANDROID_ABIS[@]}"; do
        verify_sketch_symbols "$abi" <<< "$(nm -D -g "$extract_dir/package/rust/android/$abi/libsketch_core.so" 2>/dev/null || true)" \
            || failures=$((failures + 1))
    done
fi

if [[ $failures -gt 0 ]]; then
    echo "Package validation failed with $failures problem(s)." >&2
    exit 1
fi
echo "Package validation passed: $(wc -l <<< "$contents" | tr -d ' ') files in $(basename "$tarball")."
