#!/usr/bin/env bash
# Corre tree-sitter-cli sobre grammars/tree-sitter-go en un contenedor de node (trae gcc).
# Uso: scripts/parity/go/grammar.sh generate
#      scripts/parity/go/grammar.sh test   (el corpus de upstream, de .parity/grammar/tree-sitter-go/test)
#      scripts/parity/go/grammar.sh parse archivo.go   (rutas relativas al repo, o -e 'texto')
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../.." && pwd -W)"
readonly CLI="tree-sitter-cli@0.25.8"
MSYS_NO_PATHCONV=1 docker run --rm -i -m 4g --cpus=4 -v "$repo:/src" -w /src/grammars/tree-sitter-go \
  -v "$repo/.parity/grammar/tree-sitter-go/test:/upstream-test:ro" -v kerbo-tree-sitter-npm:/root/.npm -v kerbo-tree-sitter-cache:/root/.cache \
  -e DEBUG -e TAIL node:22-bookworm bash -euo pipefail -c '
    command="$1"
    shift
    case "$command" in
      generate) npx -y '"$CLI"' generate ;;
      test)
        rm -rf /tmp/grammar && cp -r . /tmp/grammar && cp -r /upstream-test /tmp/grammar/test
        cd /tmp/grammar && npx -y '"$CLI"' test 2>&1 | tail -${TAIL:-15} ;;
      parse)
        if [ "${1:-}" = "-e" ]; then
          printf "%s" "$2" > /tmp/input.go
          npx -y '"$CLI"' parse ${DEBUG:+-d=normal} /tmp/input.go || true
        else
          for file in "$@"; do npx -y '"$CLI"' parse "/src/$file" || true; done
        fi ;;
      *) echo "uso: generate | test | parse archivo..." >&2; exit 2 ;;
    esac' bash "$@"
