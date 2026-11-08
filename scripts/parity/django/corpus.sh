#!/usr/bin/env bash
# Mide las migraciones de Django contra la referencia de reference.sh: kerbo corre sobre las apps del
# repo del corpus y compare.py cruza sus avisos con los de squawk por archivo, regla y cantidad.
# Uso: scripts/parity/django/corpus.sh [--skip-kerbo] [repo]
set -euo pipefail
unset MSYS_NO_PATHCONV
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../.." && pwd -W)"
parity="${KERBO_PARITY:-$(cd "$here/../../.." && pwd)/.parity}"
skip_kerbo=""
if [ "${1:-}" = "--skip-kerbo" ]; then
  skip_kerbo=1
  shift
fi
corpus_repo="${1:-django}"
apps_root="$(python "$here/compare.py" --apps-root "$corpus_repo" | tr -d '\r')"
mkdir -p "$here/out"
if [ -z "$skip_kerbo" ]; then
  image="$(docker images --format '{{.Repository}}:{{.Tag}}' quality-rust | head -1)"
  corpus="$("$here/../corpus_volume.sh" "$parity/corpus" kerbo-corpus)"
  MSYS_NO_PATHCONV=1 docker run --rm -m 16g --cpus=8 -v "$repo:/src:ro" -w /src -v rust-quality-cache:/cache \
    -v "$corpus:/c:ro" -v "$(cd "$here/out" && pwd -W):/o" \
    -e CARGO_HOME=/cache/cargo -e CARGO_TARGET_DIR=/cache/target/kerbo-dev -e CARGO_BUILD_JOBS=8 -e CARGO_TERM_COLOR=never -e RUSTFLAGS="-C link-arg=-fuse-ld=mold" \
    "$image" bash -euo pipefail -c '
      cargo build --profile parity --locked --bin kerbo 2>&1 | tail -1
      cd /c && /cache/target/kerbo-dev/parity/kerbo check --language python --format json --no-cache '"$apps_root"' > /o/'"$corpus_repo"'.json 2> /o/kerbo.log || true
      tail -1 /o/kerbo.log'
fi
python "$here/compare.py" "$corpus_repo"
