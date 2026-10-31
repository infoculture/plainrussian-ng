#!/bin/sh
# Собирает WASM-модули ядра и проверок ясного языка с JS-обёртками в src/wasm
# и кладёт рядом сжатый словарь rumorph (src/wasm/clarity-pkg/ru.dict.gz).
set -eu
cd "$(dirname "$0")/../../.."
# cargo из Homebrew не видит цель wasm32, поэтому ставим тулчейн rustup первым в PATH.
if command -v rustup >/dev/null 2>&1; then
  PATH="$(dirname "$(rustup which rustc)"):$PATH"
fi
cargo build -p pr-wasm -p pr-wasm-clarity --target wasm32-unknown-unknown --release
wasm-bindgen --target web --out-dir apps/web/src/wasm/pkg \
  target/wasm32-unknown-unknown/release/pr_wasm.wasm
wasm-bindgen --target web --out-dir apps/web/src/wasm/clarity-pkg \
  target/wasm32-unknown-unknown/release/pr_wasm_clarity.wasm

# Словарь лежит в исходниках крейта rumorph-dict-ru (CC BY-SA 4.0).
DICT=$(cargo metadata --format-version 1 | node -e '
  let s = ""; process.stdin.on("data", (d) => (s += d)).on("end", () => {
    const p = JSON.parse(s).packages.find((p) => p.name === "rumorph-dict-ru");
    console.log(require("path").join(require("path").dirname(p.manifest_path), "ru.dict"));
  });')
OUT=apps/web/src/wasm/clarity-pkg/ru.dict.gz
mkdir -p "$(dirname "$OUT")"
if [ ! "$OUT" -nt "$DICT" ]; then
  # Сжимает Node, а не gzip: так скрипт работает и там, где gzip нет (Windows).
  node -e 'const fs = require("fs"), zlib = require("zlib");
    fs.writeFileSync(process.argv[2], zlib.gzipSync(fs.readFileSync(process.argv[1]), { level: 9 }));' "$DICT" "$OUT"
fi
