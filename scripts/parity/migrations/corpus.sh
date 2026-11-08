#!/usr/bin/env bash
# Mide las migraciones SQL contra squawk en los repos de .parity/corpus. Squawk corre en un venv de
# .parity (squawk-venv, con la versión de versions.toml) con --assume-in-transaction, y kerbo en la
# imagen del gate sobre los mismos archivos. Deja squawk.json y kerbo.json en .parity/migrations y
# compara por archivo, línea y regla.
# Uso: scripts/parity/migrations/corpus.sh [--skip-squawk]
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd -W)"
repo="$(cd "$here/../../.." && pwd -W)"
parity="${KERBO_PARITY:-$(cd "$here/../../.." && pwd)/.parity}"
readonly CORPUS_REPOS=(lobe-chat documenso trigger-dev cal-com conductor awesome-compose)
mkdir -p "$parity/migrations"
cd "$parity/corpus"
find "${CORPUS_REPOS[@]}" -ipath '*migration*' -name '*.sql' -not -path '*/node_modules/*' | sort > "$parity/migrations/files.txt"
if [ "${1:-}" != "--skip-squawk" ]; then
  version="$(python "$here/../versions.py" version squawk)"
  venv="$parity/squawk-venv"
  [ -d "$venv" ] || python -m venv "$venv"
  "$venv/Scripts/pip" install -q "squawk-cli==$version"
  xargs "$venv/Scripts/squawk" --assume-in-transaction --no-error-on-unmatched-pattern --reporter json \
    < "$parity/migrations/files.txt" > "$parity/migrations/squawk.json" || true
fi
image="$(docker images --format '{{.Repository}}:{{.Tag}}' quality-rust | head -1)"
corpus="$("$here/../corpus_volume.sh" "$parity/corpus" kerbo-corpus)"
out="$(cd "$parity/migrations" && pwd -W)"
MSYS_NO_PATHCONV=1 docker run --rm -m 16g --cpus=8 -v "$repo:/src:ro" -w /src -v rust-quality-cache:/cache \
  -v "$corpus:/c:ro" -v "$out:/o" \
  -e CARGO_HOME=/cache/cargo -e CARGO_TARGET_DIR=/cache/target/kerbo-dev -e CARGO_BUILD_JOBS=8 -e CARGO_TERM_COLOR=never -e RUSTFLAGS="-C link-arg=-fuse-ld=mold" \
  "$image" bash -euo pipefail -c '
    cargo build --profile parity --locked --bin kerbo 2>&1 | tail -1
    cd /c && xargs /cache/target/kerbo-dev/parity/kerbo check --format json --no-cache < /o/files.txt > /o/kerbo.json 2> /o/kerbo_err.txt || true
    tail -1 /o/kerbo_err.txt'
python "$here/compare.py" "$parity/migrations"
