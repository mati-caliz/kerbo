#!/usr/bin/env bash
# Mide las reglas de Python contra ruff en los diez repos de .parity/corpus: ruff.json y kerbo.json
# en out/, comparados por archivo, línea, columna y regla.
# Uso: scripts/parity/python/corpus.sh [--skip-ruff] [--skip-kerbo] [reglas de compare.py...]
set -euo pipefail
unset MSYS_NO_PATHCONV
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../.." && pwd -W)"
parity="${KERBO_PARITY:-$(cd "$here/../../.." && pwd)/.parity}"
readonly CORPUS_PATHS=(requests fastapi flask django pydantic sqlalchemy ipython ansible sympy cpython/Lib)
mkdir -p "$here/out"
skip_ruff=""
skip_kerbo=""
while [ "${1:-}" = "--skip-ruff" ] || [ "${1:-}" = "--skip-kerbo" ]; do
  [ "$1" = "--skip-ruff" ] && skip_ruff=1
  [ "$1" = "--skip-kerbo" ] && skip_kerbo=1
  shift
done
if [ -z "$skip_ruff" ]; then
  wanted="$(python "$here/../versions.py" version ruff)"
  installed="$("$parity/ruff/ruff.exe" --version | cut -d" " -f2)"
  [ "$installed" = "$wanted" ] || { echo "ruff $installed en .parity/ruff, versions.toml pide $wanted" >&2; exit 1; }
  select_codes="$(python "$here/compare.py" --select-codes)"
  (cd "$parity/corpus" && "$parity/ruff/ruff.exe" check --isolated --no-cache --exit-zero --output-format json \
    --select "$select_codes" "${CORPUS_PATHS[@]}" > "$here/out/ruff.json")
fi
if [ -z "$skip_kerbo" ]; then
  image="$(docker images --format '{{.Repository}}:{{.Tag}}' quality-rust | head -1)"
  corpus="$("$here/../corpus_volume.sh" "$parity/corpus" kerbo-corpus)"
  MSYS_NO_PATHCONV=1 docker run --rm -m 16g --cpus=8 -v "$repo:/src:ro" -w /src -v rust-quality-cache:/cache \
    -v "$corpus:/c:ro" -v "$(cd "$here/out" && pwd -W):/o" \
    -e CARGO_HOME=/cache/cargo -e CARGO_TARGET_DIR=/cache/target/kerbo-dev -e CARGO_BUILD_JOBS=8 -e CARGO_TERM_COLOR=never -e RUSTFLAGS="-C link-arg=-fuse-ld=mold" \
    "$image" bash -euo pipefail -c '
      cargo build --profile parity --locked --bin kerbo 2>&1 | tail -1
      cd /c && /cache/target/kerbo-dev/parity/kerbo check --language python --format json --no-cache '"${CORPUS_PATHS[*]}"' > /o/kerbo.json 2> /o/kerbo.log || true
      tail -1 /o/kerbo.log'
fi
python "$here/compare.py" "$@"
