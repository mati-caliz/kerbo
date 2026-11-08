#!/usr/bin/env bash
# Mide los secretos versionados contra gitleaks en los repos de .parity/corpus. gitleaks corre en
# docker (la versión de versions.toml) con la configuración por defecto copiada en
# src/secrets/gitleaks.toml, sin historia de git y sin .gitleaksignore; kerbo corre en la imagen del
# gate sobre los mismos archivos. Deja gitleaks.json y kerbo.json en .parity/secrets y compara por
# archivo, línea y regla.
# Uso: scripts/parity/secrets/corpus.sh [--skip-gitleaks]
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd -W)"
repo="$(cd "$here/../../.." && pwd -W)"
parity="${KERBO_PARITY:-$(cd "$here/../../.." && pwd)/.parity}"
readonly CORPUS_REPOS=(detect-secrets gitleaks juice-shop spring-boot trufflehog vscode flask express requests fastapi django cpython junit4 commons-lang guava)
mkdir -p "$parity/secrets"
version="$(python "$here/../versions.py" version gitleaks)"
corpus="$("$here/../corpus_volume.sh" "$parity/corpus" kerbo-corpus)"
out="$(cd "$parity/secrets" && pwd -W)"
if [ "${1:-}" != "--skip-gitleaks" ]; then
  : > "$out/gitleaks.json"
  for name in "${CORPUS_REPOS[@]}"; do
    MSYS_NO_PATHCONV=1 docker run --rm --cpus=4 -v "$corpus:/c:ro" -v "$repo/src/secrets/gitleaks.toml:/gitleaks.toml:ro" \
      -v "$out:/o" "zricethezav/gitleaks:v$version" dir "/c/$name" --config /gitleaks.toml --no-banner \
      --gitleaks-ignore-path /dev/null --exit-code 0 --report-format json --report-path "/o/$name.json" --redact > /dev/null 2>&1 || true
  done
fi
image="$(docker images --format '{{.Repository}}:{{.Tag}}' quality-rust | head -1)"
MSYS_NO_PATHCONV=1 docker run --rm -m 16g --cpus=8 -v "$repo:/src:ro" -w /src -v rust-quality-cache:/cache \
  -v "$corpus:/c:ro" -v "$out:/o" \
  -e CARGO_HOME=/cache/cargo -e CARGO_TARGET_DIR=/cache/target/kerbo-dev -e CARGO_BUILD_JOBS=8 -e CARGO_TERM_COLOR=never -e RUSTFLAGS="-C link-arg=-fuse-ld=mold" \
  -e "REPOS=${CORPUS_REPOS[*]}" \
  "$image" bash -euo pipefail -c '
    cargo build --profile parity --locked --bin kerbo 2>&1 | tail -1
    for name in $REPOS; do
      (cd /c/$name && /cache/target/kerbo-dev/parity/kerbo check --format json --no-cache . > /o/kerbo-$name.json 2> /o/kerbo-$name.err || true)
    done'
python "$here/compare.py" "$parity/secrets" "${CORPUS_REPOS[@]}"
