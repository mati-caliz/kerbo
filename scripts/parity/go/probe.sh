#!/usr/bin/env bash
# Corre golangci-lint v2.15.0 (conjunto por defecto) y el helper de kerbo sobre testdata/staticcheck y
# muestra los avisos de staticcheck de cada uno como `regla línea:columna archivo`, ordenados.
# Antes: helper-build.sh.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd -W)"
out="$(cd "$(dirname "$0")" && pwd)/out"
mkdir -p "$out"
MSYS_NO_PATHCONV=1 docker run --rm -v "$here:/p:ro" -v gocache:/root/.cache/go-build -v gomodcache:/go/pkg/mod \
  golangci/golangci-lint:v2.15.0 bash -c '
    rm -rf /tmp/m && cp -r /p/testdata/staticcheck /tmp/m && cd /tmp/m &&
    golangci-lint run -c /p/golangci-default.yml --output.json.path=/tmp/g.json --output.text.path=/dev/null ./... >/dev/null 2>&1;
    cat /tmp/g.json' > "$out/golangci.json"
MSYS_NO_PATHCONV=1 docker run --rm -v "$here:/p:ro" -v go-parity:/w -v gocache:/root/.cache/go-build \
  -v gomodcache:/go/pkg/mod golang:1.27 bash -c '
    rm -rf /tmp/m && cp -r /p/testdata/staticcheck /tmp/m && cd /tmp/m && /w/bin/helper .' > "$out/helper.jsonl"
python "$(dirname "$0")/probe_compare.py" "$out/golangci.json" "$out/helper.jsonl"
