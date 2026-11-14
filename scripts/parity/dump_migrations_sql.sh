#!/usr/bin/env bash
# Vuelca el SQL que kerbo genera para cada migración de un repo del corpus en
# .parity/<pila>/<repo>-kerbo, corriendo el test ignorado que lo escribe en /o con el corpus montado
# en /c. Lo usan django/dump.sh y rails/dump.sh.
# Uso: scripts/parity/dump_migrations_sql.sh <pila> <repo> <test> <VARIABLE=valor>...
set -euo pipefail
unset MSYS_NO_PATHCONV
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "${here}/../.." && pwd -W)"
default_parity="$(cd "${here}/../.." && pwd)/.parity"
parity="${KERBO_PARITY:-${default_parity}}"
stack="$1"
corpus_repo="$2"
test_name="$3"
shift 3
environment=()
for assignment in "$@"; do
  environment+=(-e "${assignment}")
done
output="${parity}/${stack}/${corpus_repo}-kerbo"
rm -rf "${output}"
mkdir -p "${output}"
output_mount="$(cd "${output}" && pwd -W)"
image="$(docker images --format '{{.Repository}}:{{.Tag}}' quality-rust | head -1)"
corpus="$("${here}/corpus_volume.sh" "${parity}/corpus" kerbo-corpus)"
MSYS_NO_PATHCONV=1 docker run --rm -m 16g --cpus=8 -v "${repo}:/src:ro" -w /src -v rust-quality-cache:/cache \
  -v "${corpus}:/c:ro" -v "${output_mount}:/o" \
  -e CARGO_HOME=/cache/cargo -e CARGO_TARGET_DIR=/cache/target/kerbo-dev -e CARGO_BUILD_JOBS=8 -e CARGO_TERM_COLOR=never \
  "${environment[@]}" "${image}" \
  bash -c "cargo test --locked --bin kerbo -- --ignored ${test_name} 2>&1 | tail -5"
