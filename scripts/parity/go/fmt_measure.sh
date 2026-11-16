#!/usr/bin/env bash
# Mide `kerbo fmt` contra gofmt sobre el corpus del volumen `fmt-parity`: 14 repos de Go en tres
# versiones (`orig`, `scr` desordenado a propósito y `crlf`, el desordenado con CRLF) y, al lado, lo
# que gofmt 1.27 deja de cada una (`gofmt_<versión>`, con los archivos que no parsea en
# `gofmt_<versión>.err`). Copia cada versión, la formatea con kerbo y lista en
# `/v/diff_<versión>.txt` los archivos que no quedan idénticos; `second` formatea otra vez lo ya
# formateado de `scr` y lista lo que cambia. Corre en el volumen porque las rutas del corpus no
# entran en el largo máximo de Windows.
#
#   heavy_lock.py -- bash scripts/parity/go/fmt_measure.sh
#   heavy_lock.py -- bash scripts/parity/go/fmt_measure.sh show scr tools/go/ssa/testdata/objlookup.go
set -euo pipefail

readonly CORPUS_VOLUME="fmt-parity"
readonly CACHE_VOLUME="rust-quality-cache"
readonly CPUS=8
readonly MEMORY="16g"

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && { pwd -W 2>/dev/null || pwd; })"
image="$(docker images --format '{{.Repository}}:{{.Tag}}' quality-rust | head -1)"

run() {
  MSYS_NO_PATHCONV=1 docker run --rm -i -m "${MEMORY}" --cpus="${CPUS}" \
    -v "${repo_dir}:/src:ro" -w /src -v "${CACHE_VOLUME}:/cache" -v "${CORPUS_VOLUME}:/v" \
    -e CARGO_HOME=/cache/cargo -e CARGO_TARGET_DIR=/cache/target/kerbo-dev \
    -e CARGO_BUILD_JOBS="${CPUS}" -e CARGO_TERM_COLOR=never \
    "${image}" bash -euo pipefail -s -- "$@"
}

if [ "${1:-}" = "show" ]; then
  run "$2" "$3" <<'SHOW'
{ diff "/v/gofmt_$1/$2" "/v/kerbo_$1/$2" || true; } | head -n 60
SHOW
  exit 0
fi

run <<'MEASURE'
cargo build --locked --release 2>&1 | tail -n 1
kerbo=/cache/target/kerbo-dev/release/kerbo
for version in orig scr crlf; do
  rm -rf "/v/kerbo_${version}"
  cp -a "/v/${version}" "/v/kerbo_${version}"
  "${kerbo}" fmt "/v/kerbo_${version}" >"/v/kerbo_${version}.log" 2>&1 || true
  unparsed_by_gofmt="$(sed -n "s#^/[a-z]*/gofmt_${version}/\([^:]*\.go\):.*#\1#p" "/v/gofmt_${version}.err" | sort -u)"
  { diff -rq "/v/gofmt_${version}" "/v/kerbo_${version}" || true; } |
    sed -n "s#^Files /v/gofmt_${version}/\(.*\) and .* differ\$#\1#p" | sort |
    comm -23 - <(printf '%s\n' "${unparsed_by_gofmt}") >"/v/diff_${version}.txt"
  total="$(find "/v/${version}" -name '*.go' | wc -l)"
  echo "${version}: $(wc -l <"/v/diff_${version}.txt") distintos de ${total} ($(printf '%s\n' "${unparsed_by_gofmt}" | grep -c . || true) que gofmt no parsea, afuera); $(grep -c 'no parsea limpio\|quedó como estaba' "/v/kerbo_${version}.log" || true) que kerbo deja como están"
done
rm -rf /v/kerbo_second
cp -a /v/kerbo_scr /v/kerbo_second
"${kerbo}" fmt /v/kerbo_second >/v/kerbo_second.log 2>&1 || true
{ diff -rq /v/kerbo_scr /v/kerbo_second || true; } | sed -n 's#^Files /v/kerbo_scr/\(.*\) and .* differ$#\1#p' >/v/diff_second.txt
echo "second: $(wc -l </v/diff_second.txt) cambian al formatear otra vez"
for version in orig scr crlf second; do
  echo "== ${version}"
  cat "/v/diff_${version}.txt"
done
MEASURE
