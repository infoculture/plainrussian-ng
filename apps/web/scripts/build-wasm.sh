#!/bin/sh
# Собирает pr-wasm и JS-обёртку в src/wasm/pkg.
set -eu
cd "$(dirname "$0")/../../.."
# cargo из Homebrew не видит цель wasm32, поэтому ставим тулчейн rustup первым в PATH.
if command -v rustup >/dev/null 2>&1; then
  PATH="$(dirname "$(rustup which rustc)"):$PATH"
fi
cargo build -p pr-wasm --target wasm32-unknown-unknown --release
wasm-bindgen --target web --out-dir apps/web/src/wasm/pkg \
  target/wasm32-unknown-unknown/release/pr_wasm.wasm
