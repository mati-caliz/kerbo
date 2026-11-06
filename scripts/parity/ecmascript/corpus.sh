#!/usr/bin/env bash
# Mide reglas de JS/TS contra ESLint en los repos de kerbo-parity/corpus. ESLint corre sólo con las
# reglas pedidas, así tarda menos, y kerbo en la imagen del gate. Deja out_rules.json y kerbo.json en
# kerbo-parity/eslint y compara por archivo, línea, columna y regla.
# Uso: scripts/parity/ecmascript/corpus.sh no-octal,no-with [--skip-eslint]
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../.." && pwd -W)"
parity="$(cd "$here/../../../.." && pwd)/kerbo-parity"
rules="$1"
readonly CORPUS_REPOS=(express zod react vscode juice-shop)
cd "$parity/eslint"
if [ "${2:-}" != "--skip-eslint" ]; then
  python - "$rules" <<'PY'
import json, sys
open('kerbo_rules.txt', 'w', encoding='utf-8', newline='\n').write(','.join(json.dumps(name) for name in sys.argv[1].split(',')))
PY
  (cd "$parity/corpus" && NODE_OPTIONS=--max-old-space-size=12000 node ../eslint/node_modules/eslint/bin/eslint.js     -c ../eslint/eslint.config.mjs --no-warn-ignored -f json -o ../eslint/out_rules.json "${CORPUS_REPOS[@]}") || true
fi
image="$(docker images --format '{{.Repository}}:{{.Tag}}' quality-rust | head -1)"
corpus="$(cd "$parity/corpus" && pwd -W)"
eslint_out="$(pwd -W)"
MSYS_NO_PATHCONV=1 docker run --rm -m 16g --cpus=8 -v "$repo:/src:ro" -w /src -v rust-quality-cache:/cache \
  -v "$corpus:/c:ro" -v "$eslint_out:/o" \
  -e CARGO_HOME=/cache/cargo -e CARGO_TARGET_DIR=/cache/target/kerbo-dev -e CARGO_BUILD_JOBS=8 -e CARGO_TERM_COLOR=never \
  "$image" bash -euo pipefail -c '
    cargo build --release --locked --bin kerbo 2>&1 | tail -1
    cd /c && /cache/target/kerbo-dev/release/kerbo check --format json --no-cache '"${CORPUS_REPOS[*]}"' > /o/kerbo.json 2> /o/kerbo_err.txt || true
    tail -1 /o/kerbo_err.txt'
python rule_compare.py out_rules.json "$rules" 15
