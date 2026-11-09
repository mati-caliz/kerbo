#!/usr/bin/env bash
# Mide `check-blank` y `check-type-assertions` de errcheck contra golangci-lint v2.15.0: el helper
# (helper-build.sh antes) y golangci-lint con las dos opciones prendidas, sobre los repos del volumen
# go-parity y testdata/errcheck_options. Deja la salida en out/errcheck_options para el comparador.
# uso: errcheck_options.sh [repos...]
set -euo pipefail
versions="$(cd "$(dirname "$0")/.." && pwd -W)/versions.py"
here="$(cd "$(dirname "$0")" && pwd -W)"
out="$(cd "$(dirname "$0")" && pwd)/out/errcheck_options"
repos="${*:-chi cobra gin fzf caddy}"
rm -rf "$out" && mkdir -p "$out/helper" "$out/golangci"
MSYS_NO_PATHCONV=1 docker run --rm -v "$here:/p:ro" -v "$(cd "$out" && pwd -W):/o" -v go-parity:/w \
  -v gocache:/root/.cache/go-build -v gomodcache:/go/pkg/mod "$(python "$versions" image golangci-lint)" bash -c '
    rm -rf /w/repos/errcheck_options && cp -r /p/testdata/errcheck_options /w/repos/errcheck_options
    for repo in errcheck_options '"$repos"'; do
      for mod in $(cd /w/repos/$repo && find . -name go.mod -not -path "*/testdata/*" -not -path "*/vendor/*" | sort); do
        dir=$(dirname $mod); tag=$(echo "$repo/$dir" | tr "/." "__")
        (cd /w/repos/$repo/$dir &&
          GOOS=linux GOARCH=amd64 golangci-lint run -c /p/golangci-errcheck-options.yml \
            --output.json.path=/o/golangci/$tag.json --output.text.path=/dev/null ./... >/dev/null 2>&1) || true
      done
    done'
MSYS_NO_PATHCONV=1 docker run --rm -v "$(cd "$out" && pwd -W):/o" -v go-parity:/w \
  -v gocache:/root/.cache/go-build -v gomodcache:/go/pkg/mod "$(python "$versions" image go)" bash -c '
    for repo in errcheck_options '"$repos"'; do
      for mod in $(cd /w/repos/$repo && find . -name go.mod -not -path "*/testdata/*" -not -path "*/vendor/*" | sort); do
        dir=$(dirname $mod); tag=$(echo "$repo/$dir" | tr "/." "__")
        (cd /w/repos/$repo/$dir && GOOS=linux GOARCH=amd64 CGO_ENABLED=1 /w/bin/helper . > /o/helper/$tag.jsonl 2>/dev/null) || true
      done
    done'
PYTHONIOENCODING=utf-8 python "$(dirname "$0")/compare_errcheck_options.py" "$out"
