#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

echo "==> Rebuilding Rust artifacts and generated bindings..."
npm run build:rust

echo "==> Checking third-party notices..."
npm run notices:check

echo "==> Building and packing the JavaScript package..."
pack_dir="$(mktemp -d)"
trap 'rm -rf "$pack_dir"' EXIT
tarball="$pack_dir/$(npm pack --silent --pack-destination "$pack_dir")"

echo "==> Validating the packed package..."
bash ./scripts/validate-packed-package.sh "$tarball"

echo "==> Package publish prep complete."
