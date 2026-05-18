#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

OUT_DIR="${1:-dist}"
WASM_TARGET="wasm32-unknown-unknown"
WASM_EXAMPLE="orbifold_web"

if ! command -v wasm-bindgen >/dev/null 2>&1; then
  echo "wasm-bindgen is required. Install with: cargo install wasm-bindgen-cli --version 0.2.121 --locked" >&2
  exit 1
fi

rm -rf "$OUT_DIR"
mkdir -p "$OUT_DIR/pkg"

cargo build \
  --release \
  --target "$WASM_TARGET" \
  --example "$WASM_EXAMPLE" \
  --no-default-features \
  --features web-app

wasm-bindgen \
  --target web \
  --out-dir "$OUT_DIR/pkg" \
  --out-name "$WASM_EXAMPLE" \
  "target/$WASM_TARGET/release/examples/$WASM_EXAMPLE.wasm"

cp web/index.html "$OUT_DIR/index.html"
cp favicon.ico "$OUT_DIR/favicon.ico"
cp orbifold_icon.png "$OUT_DIR/orbifold_icon.png"
: > "$OUT_DIR/.nojekyll"
