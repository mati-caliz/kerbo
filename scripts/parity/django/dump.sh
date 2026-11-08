#!/usr/bin/env bash
# Vuelca el SQL que kerbo genera para cada migración de un repo del corpus en
# .parity/django/<repo>-kerbo/<app>/<migración>.sql, para cruzarlo con el de sqlmigrate (diff.py).
# Uso: scripts/parity/django/dump.sh [repo]
set -euo pipefail
unset MSYS_NO_PATHCONV
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../.." && pwd -W)"
parity="${KERBO_PARITY:-$(cd "$here/../../.." && pwd)/.parity}"
corpus_repo="${1:-django}"
apps_root="$(python "$here/compare.py" --apps-root "$corpus_repo" | tr -d '\r')"
output="$parity/django/$corpus_repo-kerbo"
rm -rf "$output"
mkdir -p "$output"
image="$(docker images --format '{{.Repository}}:{{.Tag}}' quality-rust | head -1)"
corpus="$("$here/../corpus_volume.sh" "$parity/corpus" kerbo-corpus)"
MSYS_NO_PATHCONV=1 docker run --rm -m 16g --cpus=8 -v "$repo:/src:ro" -w /src -v rust-quality-cache:/cache \
  -v "$corpus:/c:ro" -v "$(cd "$output" && pwd -W):/o" \
  -e CARGO_HOME=/cache/cargo -e CARGO_TARGET_DIR=/cache/target/kerbo-dev -e CARGO_BUILD_JOBS=8 -e CARGO_TERM_COLOR=never \
  -e KERBO_DJANGO_APPS_ROOT="/c/$apps_root" -e KERBO_DJANGO_SQL_OUTPUT=/o "$image" \
  bash -c 'cargo test --locked --bin kerbo -- --ignored dumps_the_sql_of_every_migration 2>&1 | tail -5'
