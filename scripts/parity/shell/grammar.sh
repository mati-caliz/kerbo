#!/usr/bin/env bash
# Corre tree-sitter-cli sobre grammars/tree-sitter-bash en un contenedor de node (trae gcc).
# Uso: scripts/parity/shell/grammar.sh generate
#      scripts/parity/shell/grammar.sh test   (el corpus de upstream, de .parity/grammar/ts-bash/test)
#      scripts/parity/shell/grammar.sh parse archivo.sh   (rutas relativas al repo, o -e 'texto')
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../.." && pwd -W)"
readonly CLI="tree-sitter-cli@0.25.9"
MSYS_NO_PATHCONV=1 docker run --rm -i -m 4g --cpus=4 -v "$repo:/src" -w /src/grammars/tree-sitter-bash \
  -v "$repo/.parity/grammar/ts-bash/test:/upstream-test:ro" -v kerbo-tree-sitter-npm:/root/.npm -v kerbo-tree-sitter-cache:/root/.cache \
  -e DEBUG node:22-bookworm bash -euo pipefail -c '
    command="$1"
    shift
    case "$command" in
      generate) npx -y '"$CLI"' generate ;;
      test)
        rm -rf /tmp/grammar && cp -r . /tmp/grammar && cp -r /upstream-test /tmp/grammar/test
        cd /tmp/grammar && npx -y '"$CLI"' test 2>&1 | tail -15 ;;
      parse)
        if [ "${1:-}" = "-e" ]; then
          printf "%s" "$2" > /tmp/input.sh
          npx -y '"$CLI"' parse ${DEBUG:+-d=normal} /tmp/input.sh || true
        else
          for file in "$@"; do npx -y '"$CLI"' parse "/src/$file" || true; done
        fi ;;
      *) echo "uso: generate | test | parse archivo..." >&2; exit 2 ;;
    esac' bash "$@"
