#!/usr/bin/env bash
# Mide las vulnerabilidades de dependencias contra osv-scanner en los lockfiles de .parity/corpus.
# osv-scanner corre en docker (la versión de versions.toml) con -L por cada lockfile y kerbo en la
# imagen del gate con --osv sobre los mismos archivos; los dos consultan api.osv.dev, así que los
# resultados dependen del día. Deja osv-scanner.json y kerbo.json en .parity/dependencies y compara
# por carpeta, paquete y versión (osv-scanner informa una sola vez un paquete que aparece en dos
# lockfiles de la misma carpeta), con los avisos agrupados como los agrupa cada herramienta.
# Uso: scripts/parity/dependencies/corpus.sh [--skip-osv-scanner]
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd -W)"
repo="$(cd "$here/../../.." && pwd -W)"
parity="${KERBO_PARITY:-$(cd "$here/../../.." && pwd)/.parity}"
readonly LOCKFILE_NAMES=(package-lock.json npm-shrinkwrap.json pnpm-lock.yaml yarn.lock Cargo.lock go.mod poetry.lock uv.lock Pipfile.lock gradle.lockfile)
mkdir -p "$parity/dependencies"
version="$(python "$here/../versions.py" version osv-scanner)"
corpus="$("$here/../corpus_volume.sh" "$parity/corpus" kerbo-corpus)"
out="$(cd "$parity/dependencies" && pwd -W)"
names_expression="$(printf -- '-name %s -o ' "${LOCKFILE_NAMES[@]}" | sed 's/ -o $//')"
MSYS_NO_PATHCONV=1 docker run --rm -v "$corpus:/c:ro" -v "$out:/o" alpine:3.22 sh -c \
  "cd /c && find . \( $names_expression \) -not -path '*/node_modules/*' | sed 's|^\./||' | sort > /o/files.txt"
if [ "${1:-}" != "--skip-osv-scanner" ]; then
  arguments=()
  while read -r lockfile; do arguments+=(-L "/c/$lockfile"); done < "$out/files.txt"
  MSYS_NO_PATHCONV=1 docker run --rm --cpus=4 -v "$corpus:/c:ro" "ghcr.io/google/osv-scanner:v$version" \
    scan source --format json "${arguments[@]}" > "$out/osv-scanner.json" 2> "$out/osv-scanner.err" || true
fi
image="$(docker images --format '{{.Repository}}:{{.Tag}}' quality-rust | head -1)"
MSYS_NO_PATHCONV=1 docker run --rm -m 16g --cpus=8 -v "$repo:/src:ro" -w /src -v rust-quality-cache:/cache \
  -v "$corpus:/c:ro" -v "$out:/o" \
  -e CARGO_HOME=/cache/cargo -e CARGO_TARGET_DIR=/cache/target/kerbo-dev -e CARGO_BUILD_JOBS=8 -e CARGO_TERM_COLOR=never -e RUSTFLAGS="-C link-arg=-fuse-ld=mold" \
  "$image" bash -euo pipefail -c '
    command -v curl > /dev/null || { apt-get update -qq && apt-get install -y -qq curl > /dev/null; }
    cargo build --profile parity --locked --bin kerbo 2>&1 | tail -1
    cd /c && xargs /cache/target/kerbo-dev/parity/kerbo check --osv --format json --no-cache < /o/files.txt > /o/kerbo.json 2> /o/kerbo_err.txt || true
    tail -1 /o/kerbo_err.txt'
python "$here/compare.py" "$parity/dependencies"
