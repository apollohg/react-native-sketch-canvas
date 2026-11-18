#!/usr/bin/env bash
#
# Usage: rust/build-wasm.sh [version]
# Builds sketch-core for the web into rust/wasm, an npm package directory. The version defaults
# to the sketch-core crate version.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
CRATE_DIR="$SCRIPT_DIR/sketch-wasm"
OUT_DIR="$SCRIPT_DIR/wasm"
TARGET="wasm32-unknown-unknown"
OUT_NAME="sketch_core_wasm"
PACKAGE_NAME="@apollohg/sketch-core-wasm"
REPOSITORY_URL="git+https://github.com/apollohg/react-native-sketch-canvas.git"
REGISTRY_URL="https://npm.pkg.github.com"
SEMVER_PATTERN='^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$'

source "$SCRIPT_DIR/toolchain.sh"
source "$SCRIPT_DIR/wasm-bindgen.sh"
wasm_bindgen_resolve
TARGET_DIR="${CARGO_TARGET_DIR:-$CRATE_DIR/target}"

if [[ $# -gt 0 ]]; then
    VERSION="$1"
else
    CORE_PKGID="$("$RUST_TOOLCHAIN_CARGO" pkgid --manifest-path "$SCRIPT_DIR/sketch-core/Cargo.toml")"
    VERSION="${CORE_PKGID##*[#@]}"
fi
[[ "$VERSION" =~ $SEMVER_PATTERN ]] || { echo "error: '$VERSION' is not a semver version" >&2; exit 1; }

echo "==> Building sketch-wasm for $TARGET..."
"$RUST_TOOLCHAIN_CARGO" build --manifest-path "$CRATE_DIR/Cargo.toml" --locked --release \
    --target "$TARGET" --target-dir "$TARGET_DIR"

echo "==> Generating the ES module with $("$WASM_BINDGEN" --version)..."
rm -rf "$OUT_DIR"
"$WASM_BINDGEN" --target web --out-dir "$OUT_DIR" --out-name "$OUT_NAME" \
    "$TARGET_DIR/$TARGET/release/sketch_wasm.wasm"

echo "==> Writing $PACKAGE_NAME@$VERSION..."
cp "$REPO_DIR/LICENSE" "$REPO_DIR/RUST-STANDARD-LIBRARY-NOTICES.html" "$OUT_DIR/"
cp "$CRATE_DIR/THIRD_PARTY_NOTICES.md" "$OUT_DIR/"
cat > "$OUT_DIR/package.json" <<EOF
{
    "name": "$PACKAGE_NAME",
    "version": "$VERSION",
    "description": "WebAssembly build of the sketch canvas Rust core",
    "license": "Apache-2.0",
    "repository": {
        "type": "git",
        "url": "$REPOSITORY_URL",
        "directory": "rust/sketch-wasm"
    },
    "type": "module",
    "main": "./$OUT_NAME.js",
    "module": "./$OUT_NAME.js",
    "types": "./$OUT_NAME.d.ts",
    "exports": {
        ".": {
            "types": "./$OUT_NAME.d.ts",
            "default": "./$OUT_NAME.js"
        },
        "./${OUT_NAME}_bg.wasm": "./${OUT_NAME}_bg.wasm",
        "./package.json": "./package.json"
    },
    "files": [
        "$OUT_NAME.js",
        "$OUT_NAME.d.ts",
        "${OUT_NAME}_bg.wasm",
        "LICENSE",
        "THIRD_PARTY_NOTICES.md",
        "RUST-STANDARD-LIBRARY-NOTICES.html"
    ],
    "sideEffects": false,
    "publishConfig": {
        "registry": "$REGISTRY_URL"
    }
}
EOF

echo "==> Web package complete: $OUT_DIR"
