#!/usr/bin/env bash
# Corre los tests del módulo de Django de kerbo en Docker, sin el gate completo.
# Uso: scripts/parity/django/test.sh [filtro]
set -euo pipefail
unset MSYS_NO_PATHCONV
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../.." && pwd -W)"
filter="${1:-deploy::django}"
image="$(docker images --format '{{.Repository}}:{{.Tag}}' quality-rust | head -1)"
MSYS_NO_PATHCONV=1 docker run --rm -m 16g --cpus=8 -v "$repo:/src:ro" -w /src -v rust-quality-cache:/cache \
  -e CARGO_HOME=/cache/cargo -e CARGO_TARGET_DIR=/cache/target/kerbo-dev -e CARGO_BUILD_JOBS=8 -e CARGO_TERM_COLOR=never \
  "$image" bash -c "cargo test --locked --bin kerbo -- $filter 2>&1 | grep -E '^(error|warning|test result|failures:|---- |  left| right)|panicked|falta' | head -60"
