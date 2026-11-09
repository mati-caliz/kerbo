#!/usr/bin/env bash
# Corre golangci-lint (la versión de versions.toml, conjunto por defecto) en cada módulo de los 12
# repos del volumen go-parity y deja la salida en /w/default (un .json y un .log por módulo), que
# corpus-helper.sh copia a out/default para compare_corpus.py.
set -euo pipefail
versions="$(cd "$(dirname "$0")/.." && pwd -W)/versions.py"
here="$(cd "$(dirname "$0")" && pwd -W)"
MSYS_NO_PATHCONV=1 docker run --rm --cpus=4 -m 8g -v go-parity:/w -v "$here:/p:ro" -v gocache:/root/.cache/go-build \
  -v gomodcache:/go/pkg/mod "$(python "$versions" image golangci-lint)" bash -c '
    rm -rf /w/default && mkdir -p /w/default
    for repo in chi cobra gin fzf caddy hugo tools prometheus terraform syncthing etcd consul; do
      for mod in $(cd /w/repos/$repo && find . -name go.mod -not -path "*/testdata/*" -not -path "*/vendor/*" | sort); do
        dir=$(dirname $mod); tag=$(echo "$repo/$dir" | tr "/." "__")
        (cd /w/repos/$repo/$dir && GOOS=linux GOARCH=amd64 golangci-lint run -c /p/golangci-default.yml \
          --output.json.path=/w/default/$tag.json --output.text.path=/dev/null ./... > /w/default/$tag.log 2>&1) || true
      done
    done'
