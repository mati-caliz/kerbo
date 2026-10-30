#!/usr/bin/env bash
# Corre ShellCheck 0.11.0 y kerbo (compilado en release) sobre testdata/<carpeta> y compara los
# avisos cuyo código empieza con el prefijo (por defecto, todos).
# Uso: scripts/parity/shell/probe.sh bashisms [sc30]
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
folder="$1"
prefix="${2:-sc}"
repo="$(cd "$here/../../.." && pwd -W)"
data="$(cd "$here/testdata/$folder" && pwd -W)"
mkdir -p "$here/out"
out="$(cd "$here/out" && pwd -W)"
MSYS_NO_PATHCONV=1 docker run --rm -v "$data:/p:ro" -v "$out:/o" koalaman/shellcheck-alpine:v0.11.0 \
  sh -c 'cd /p && shellcheck -f json1 *.sh > /o/probe-shellcheck.json; exit 0'
image="$(docker images --format '{{.Repository}}:{{.Tag}}' quality-rust | head -1)"
MSYS_NO_PATHCONV=1 docker run --rm -m 16g --cpus=8 -v "$repo:/src:ro" -w /src -v rust-quality-cache:/cache \
  -v "$data:/p:ro" -v "$out:/o" \
  -e CARGO_HOME=/cache/cargo -e CARGO_TARGET_DIR=/cache/target/kerbo-dev -e CARGO_BUILD_JOBS=8 -e CARGO_TERM_COLOR=never \
  "$image" bash -euo pipefail -c '
    cargo build --release --locked --bin kerbo 2>&1 | grep -E "^(error|warning: unused)" -A5 || true
    rm -rf /tmp/p && cp -r /p /tmp/p && cd /tmp/p
    /cache/target/kerbo-dev/release/kerbo check --language shell --format json --no-cache . > /o/probe-kerbo.json 2>/dev/null || true'
python "$here/probe_compare.py" "$here/out" "$here/testdata/$folder" "$prefix"
