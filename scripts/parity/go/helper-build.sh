#!/usr/bin/env bash
# Compila el helper de tipos de Go (src/go_types) en la imagen de Go de versions.toml y lo deja en el volumen go-parity
# como /w/bin/helper. Las cachés de Go viven en los volúmenes gocache y gomodcache.
set -euo pipefail
versions="$(cd "$(dirname "$0")/.." && pwd)/versions.py"
repo="$(cd "$(dirname "$0")/../../.." && pwd -W)"
MSYS_NO_PATHCONV=1 docker run --rm \
  -v go-parity:/w -v gocache:/root/.cache/go-build -v gomodcache:/go/pkg/mod \
  -v "$repo/src/go_types:/src:ro" "$(python "$versions" image go)" bash -c '
    set -e
    rm -rf /tmp/h && mkdir -p /tmp/h /w/bin
    cp /src/*.go /tmp/h/ && mv /tmp/h/helper.go /tmp/h/main.go
    printf "module kerbo-go-types\n\ngo 1.22\n" > /tmp/h/go.mod
    cd /tmp/h && go vet . && go build -o /w/bin/helper .'
