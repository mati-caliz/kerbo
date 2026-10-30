#!/usr/bin/env bash
# Corre el helper (helper-build.sh) en cada módulo de los 12 repos del volumen go-parity y deja la
# salida en /w/helper; después la copia a out/helper para compare_corpus.py.
set -euo pipefail
out="$(cd "$(dirname "$0")" && pwd)/out"
mkdir -p "$out/helper"
MSYS_NO_PATHCONV=1 docker run --rm -v go-parity:/w -v gocache:/root/.cache/go-build -v gomodcache:/go/pkg/mod \
  golang:1.27 bash -c '
    rm -rf /w/helper && mkdir -p /w/helper
    for repo in chi cobra gin fzf caddy hugo tools prometheus terraform syncthing etcd consul; do
      for mod in $(cd /w/repos/$repo && find . -name go.mod -not -path "*/testdata/*" -not -path "*/vendor/*" | sort); do
        dir=$(dirname $mod); tag=$(echo "$repo/$dir" | tr "/." "__")
        (cd /w/repos/$repo/$dir && GOOS=linux GOARCH=amd64 CGO_ENABLED=1 /w/bin/helper . > /w/helper/$tag.jsonl 2>/dev/null) || true
      done
    done'
MSYS_NO_PATHCONV=1 docker run --rm -v go-parity:/w -v "$(cd "$out" && pwd -W):/o" alpine sh -c 'rm -rf /o/helper && cp -r /w/helper /o/helper && rm -rf /o/default && cp -r /w/default /o/default'
