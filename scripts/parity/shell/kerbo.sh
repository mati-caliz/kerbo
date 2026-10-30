#!/usr/bin/env bash
# Compila kerbo en release en la imagen del gate y lo corre sobre el corpus de shell: out/kerbo.json.
# Con --no-directives usa la copia sin directivas `# shellcheck` que arma shellcheck-inner.sh.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../.." && pwd -W)"
corpus="${SHELL_CORPUS:-$(cd "$here/../../../.." && pwd)/kerbo-parity/shell-corpus}"
image="$(docker images --format '{{.Repository}}:{{.Tag}}' quality-rust | head -1)"
MSYS_NO_PATHCONV=1 docker run --rm -m 16g --cpus=8 -v "$repo:/src:ro" -w /src -v rust-quality-cache:/cache \
  -v "$(cd "$corpus" && pwd -W):/c:ro" -v "$(cd "$here/out" && pwd -W):/o" \
  -e CARGO_HOME=/cache/cargo -e CARGO_TARGET_DIR=/cache/target/kerbo-dev -e CARGO_BUILD_JOBS=8 -e CARGO_TERM_COLOR=never \
  "$image" bash -euo pipefail -c '
    cargo build --release --locked --bin kerbo 2>&1 | tail -2
    kerbo=/cache/target/kerbo-dev/release/kerbo
    rm -rf /tmp/c && cp -r /c /tmp/c && find /tmp/c -name .git -prune -exec rm -rf {} +
    if [ "'"${1:-}"'" = "--no-directives" ]; then
      tr -d "\r" < /o/files.txt | while read -r file; do sed -i "/^[[:space:]]*#[[:space:]]*shellcheck[[:space:]]/d" "/tmp/c/$file"; done
    fi
    cd /tmp/c && $kerbo check --language shell --format json --no-cache . > /o/kerbo.json 2> /o/kerbo.log || true
    tail -1 /o/kerbo.log'
