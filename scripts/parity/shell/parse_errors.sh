#!/usr/bin/env bash
# Dónde la gramática de bash de kerbo no lee cada script del corpus (o de testdata/ con --testdata).
# Uso: scripts/parity/shell/parse_errors.sh 'ohmyzsh/plugins/gitfast/git-completion.bash' ...
#      scripts/parity/shell/parse_errors.sh --all   (todos los de out/files.txt)
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../.." && pwd -W)"
corpus="${SHELL_CORPUS:-$(cd "$here/../../../.." && pwd)/kerbo-parity/shell-corpus}"
root=/c
if [ "${1:-}" = "--testdata" ]; then
  shift
  root=/t
fi
image="$(docker images --format '{{.Repository}}:{{.Tag}}' quality-rust | head -1)"
MSYS_NO_PATHCONV=1 docker run --rm -m 8g --cpus=4 -v "$repo:/src:ro" -w /src -v rust-quality-cache:/cache \
  -v "$(cd "$corpus" && pwd -W):/c:ro" -v "$(cd "$here/testdata" && pwd -W):/t:ro" \
  -v "$(cd "$here/out" && pwd -W):/o:ro" \
  -e CARGO_HOME=/cache/cargo -e CARGO_TARGET_DIR=/cache/target/kerbo-dev -e CARGO_BUILD_JOBS=4 -e CARGO_TERM_COLOR=never \
  "$image" bash -euo pipefail -c '
    cargo build --release --locked --example shell_parse_errors 2>&1 | grep -E "^(error|warning)" -A5 || true
    cd "$0"
    if [ "${1:-}" = "--all" ]; then
      tr -d "\r" < /o/files.txt | tr "\n" "\0" | xargs -0 /cache/target/kerbo-dev/release/examples/shell_parse_errors
    else
      /cache/target/kerbo-dev/release/examples/shell_parse_errors "$@"
    fi' "$root" "$@"
