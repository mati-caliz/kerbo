#!/usr/bin/env bash
# Corre la paridad de arreglos de Python dejando lo que arregla kerbo en target/fix-dump y muestra,
# fixture por fixture, el diff entre ruff (`.fixed`/`.unsafe`) y kerbo.
# uso: scripts/parity/fix-dump.sh [líneas de diff por fixture]
set -uo pipefail
repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
crate="$(cd "${repo}" && { pwd -W 2>/dev/null || pwd; })"
rm -rf "${repo}/target/fix-dump" && mkdir -p "${repo}/target/fix-dump"
MSYS_NO_PATHCONV=1 docker run --rm -i -m 16g --cpus=8 -v "${crate}:/src" -w /src -v rust-quality-cache:/cache \
  -e CARGO_HOME=/cache/cargo -e CARGO_TARGET_DIR=/cache/target/kerbo -e CARGO_BUILD_JOBS=8 -e CARGO_TERM_COLOR=never \
  -e KERBO_FIX_DUMP=/src/target/fix-dump \
  quality-rust:cd66e86ce930 bash -c "cargo fmt --all && cargo test --locked --bin kerbo -- fix_parity 2>&1 | grep -E '^error|^test result|panicked' | head -20"
python -I "${repo}/scripts/parity/fix_diff.py" "${1:-30}"
