#!/usr/bin/env bash
# Tests de kerbo filtrados, en el contenedor del gate y con el repo montado con escritura (corre cargo fmt).
# uso: scripts/parity/test-python.sh <filtro de test> [líneas]
crate="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && { pwd -W 2>/dev/null || pwd; })"
MSYS_NO_PATHCONV=1 docker run --rm -i -m 16g --cpus=8 -v "${crate}:/src" -w /src -v rust-quality-cache:/cache \
  -e CARGO_HOME=/cache/cargo -e CARGO_TARGET_DIR=/cache/target/kerbo -e CARGO_BUILD_JOBS=8 -e CARGO_TERM_COLOR=never \
  quality-rust:cd66e86ce930 bash -c "cargo fmt --all && cargo test --locked --bin kerbo -- '$1' 2>&1 | tail -n ${2:-60}"
