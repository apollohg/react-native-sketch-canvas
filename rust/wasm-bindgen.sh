#!/usr/bin/env bash
#
# Source this file for the wasm-bindgen CLI matching the version locked by rust/sketch-wasm.
# `wasm_bindgen_locked_version` prints that version; `wasm_bindgen_resolve` exports WASM_BINDGEN.

WASM_BINDGEN_LOCKFILE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/sketch-wasm/Cargo.lock"

wasm_bindgen_error() {
    echo "error: $*" >&2
    return 1
}

wasm_bindgen_locked_version() {
    local version
    version="$(sed -n '/^name = "wasm-bindgen"$/{n;s/^version = "\(.*\)"$/\1/p;}' "$WASM_BINDGEN_LOCKFILE")"
    [[ -n "$version" ]] || wasm_bindgen_error "wasm-bindgen is not locked in $WASM_BINDGEN_LOCKFILE" || return 1
    echo "$version"
}

wasm_bindgen_resolve() {
    local binary_path version version_output

    version="$(wasm_bindgen_locked_version)" || return 1
    binary_path="${WASM_BINDGEN:-$(command -v wasm-bindgen || true)}"
    [[ -n "$binary_path" && -x "$binary_path" ]] || \
        wasm_bindgen_error "wasm-bindgen $version is required: cargo install --locked wasm-bindgen-cli --version $version" || return 1
    version_output="$("$binary_path" --version 2>&1)" || \
        wasm_bindgen_error "failed to read the wasm-bindgen version from $binary_path" || return 1
    [[ "$version_output" == "wasm-bindgen $version" ]] || \
        wasm_bindgen_error "rust/sketch-wasm locks wasm-bindgen $version (found: $version_output at $binary_path)" || return 1

    export WASM_BINDGEN="$binary_path"
}
