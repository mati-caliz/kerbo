#!/usr/bin/env bash
# Mide las reglas de Dockerfile contra hadolint en los Dockerfile de .parity/corpus. hadolint corre
# en docker (la versión de versions.toml, con --no-fail y sólo se comparan sus reglas DL) y kerbo en
# la imagen del gate sobre los mismos archivos. Deja hadolint.json y kerbo.json en .parity/dockerfile
# y compara por archivo, línea y regla.
# Uso: scripts/parity/dockerfile/corpus.sh [--skip-hadolint]
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd -W)"
repo="$(cd "$here/../../.." && pwd -W)"
parity="${KERBO_PARITY:-$(cd "$here/../../.." && pwd)/.parity}"
mkdir -p "$parity/dockerfile"
version="$(python "$here/../versions.py" version hadolint)"
corpus="$("$here/../corpus_volume.sh" "$parity/corpus" kerbo-corpus)"
out="$(cd "$parity/dockerfile" && pwd -W)"
MSYS_NO_PATHCONV=1 docker run --rm -v "$corpus:/c:ro" -v "$out:/o" alpine:3.22 sh -c \
  'cd /c && find . -type f \( -iname "Dockerfile" -o -iname "Dockerfile.*" -o -iname "*.dockerfile" \) -not -path "*/node_modules/*" | sed "s|^\./||" | sort > /o/files.txt'
if [ "${1:-}" != "--skip-hadolint" ]; then
  mapfile -t dockerfiles < "$out/files.txt"
  MSYS_NO_PATHCONV=1 docker run --rm --cpus=4 -v "$corpus:/c:ro" -w /c "hadolint/hadolint:v$version"     hadolint --no-fail --format json "${dockerfiles[@]}" > "$out/hadolint.json" || true
fi
image="$(docker images --format '{{.Repository}}:{{.Tag}}' quality-rust | head -1)"
MSYS_NO_PATHCONV=1 docker run --rm -m 16g --cpus=8 -v "$repo:/src:ro" -w /src -v rust-quality-cache:/cache \
  -v "$corpus:/c:ro" -v "$out:/o" \
  -e CARGO_HOME=/cache/cargo -e CARGO_TARGET_DIR=/cache/target/kerbo-dev -e CARGO_BUILD_JOBS=8 -e CARGO_TERM_COLOR=never -e RUSTFLAGS="-C link-arg=-fuse-ld=mold" \
  "$image" bash -euo pipefail -c '
    cargo build --profile parity --locked --bin kerbo 2>&1 | tail -1
    cd /c && xargs /cache/target/kerbo-dev/parity/kerbo check --format json --no-cache < /o/files.txt > /o/kerbo.json 2> /o/kerbo_err.txt || true
    tail -1 /o/kerbo_err.txt'
python "$here/compare.py" "$parity/dockerfile"
