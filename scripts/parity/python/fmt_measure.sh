#!/usr/bin/env bash
# Mide `kerbo fmt` de Python contra `ruff format` sobre los `.py` del corpus de Python, en el
# volumen `fmt-parity-python`: `/v/orig` (los archivos como están), `/v/ruff_orig` (lo que deja
# `ruff format --isolated`, con sus errores en `/v/ruff_orig.err`) y `/v/kerbo_orig` (lo que deja
# kerbo, con su log en `/v/kerbo_orig.log`), y lo mismo con `crlf` (el corpus con CRLF) y `second` (lo que deja ruff, formateado otra vez). Mientras el formateador esté incompleto, un archivo con
# sintaxis que kerbo todavía no formatea queda como está y no cuenta como distinto: cuenta como
# pendiente. Distinto es el que kerbo escribió y no quedó igual que el de ruff.
#
#   heavy_lock.py -- bash scripts/parity/python/fmt_measure.sh prepare   (una vez, o al subir ruff)
#   heavy_lock.py -- bash scripts/parity/python/fmt_measure.sh
#   heavy_lock.py -- bash scripts/parity/python/fmt_measure.sh show django/django/apps/config.py
set -euo pipefail
unset MSYS_NO_PATHCONV

readonly WORK_VOLUME="fmt-parity-python"
readonly CACHE_VOLUME="rust-quality-cache"
readonly CPUS=8
readonly MEMORY="16g"
readonly CORPUS_PATHS="requests fastapi flask django pydantic sqlalchemy ipython ansible sympy cpython/Lib"

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_dir="$(cd "$here/../../.." && { pwd -W 2>/dev/null || pwd; })"
parity="${KERBO_PARITY:-$(cd "$here/../../.." && pwd)/.parity}"

if [ "${1:-}" = "prepare" ]; then
  ruff_image="ghcr.io/astral-sh/ruff:$(python "$here/../versions.py" version ruff)-alpine"
  corpus="$("$here/../corpus_volume.sh" "$parity/corpus" kerbo-corpus)"
  MSYS_NO_PATHCONV=1 docker run --rm -i -m "${MEMORY}" --cpus="${CPUS}" -v "${corpus}:/c:ro" -v "${WORK_VOLUME}:/v" \
    -e CORPUS_PATHS="${CORPUS_PATHS}" --entrypoint sh "${ruff_image}" -eu -s <<'PREPARE'
rm -rf /v/orig /v/ruff_orig
mkdir -p /v/orig
cd /c
for path in ${CORPUS_PATHS}; do
  find "${path}" -name '*.py' -type f | while read -r file; do
    mkdir -p "/v/orig/$(dirname "${file}")"
    cp "${file}" "/v/orig/${file}"
  done
done
cp -a /v/orig /v/ruff_orig
cd /v/ruff_orig
ruff format --isolated --no-cache --config "exclude = []" . >/v/ruff_orig.out 2>/v/ruff_orig.err || true
tail -n 1 /v/ruff_orig.out
rm -rf /v/crlf /v/ruff_crlf /v/second /v/ruff_second
cp -a /v/orig /v/crlf
find /v/crlf -name '*.py' -type f -exec sed -i 's/$//' {} +
cp -a /v/crlf /v/ruff_crlf
(cd /v/ruff_crlf && ruff format --isolated --no-cache --config "exclude = []" . >/v/ruff_crlf.out 2>/v/ruff_crlf.err) || true
cp -a /v/ruff_orig /v/second
cp -a /v/ruff_orig /v/ruff_second
(cd /v/ruff_second && ruff format --isolated --no-cache --config "exclude = []" . >/v/ruff_second.out 2>/v/ruff_second.err) || true
echo "$(find /v/orig -name '*.py' | wc -l) archivos; $(grep -c '^error' /v/ruff_orig.err || true) que ruff no formatea"
PREPARE
  exit 0
fi

image="$(docker images --format '{{.Repository}}:{{.Tag}}' quality-rust | head -1)"
target_dir="/cache/target/$(basename "${repo_dir}")"
run() {
  MSYS_NO_PATHCONV=1 docker run --rm -i -m "${MEMORY}" --cpus="${CPUS}" \
    -v "${repo_dir}:/src:ro" -w /src -v "${CACHE_VOLUME}:/cache" -v "${WORK_VOLUME}:/v" \
    -e CARGO_HOME=/cache/cargo -e CARGO_TARGET_DIR="${target_dir}" -e KERBO="${target_dir}/parity/kerbo" \
    -e CARGO_BUILD_JOBS="${CPUS}" -e CARGO_TERM_COLOR=never -e RUSTFLAGS="-C link-arg=-fuse-ld=mold" \
    "${image}" bash -euo pipefail -s -- "$@"
}

if [ "${1:-}" = "show" ]; then
  run "$2" <<'SHOW'
{ diff "/v/ruff_orig/$1" "/v/kerbo_orig/$1" || true; } | head -n 80
SHOW
  exit 0
fi

run <<'MEASURE'
cargo build --profile parity --locked --bin kerbo 2>&1 | tail -n 1
kerbo="${KERBO}"
for version in orig crlf second; do
  rm -rf "/v/kerbo_${version}"
  cp -a "/v/${version}" "/v/kerbo_${version}"
  (cd "/v/kerbo_${version}" && "${kerbo}" fmt --python . >"/v/kerbo_${version}.log" 2>&1) || true
  sed -n 's#^kerbo: \(.*\.py\):[0-9]*: .*quedó como estaba$#\1#p' "/v/kerbo_${version}.log" | sed 's#^\./##' | sort -u >"/v/untouched_${version}.txt"
  { diff -rq "/v/ruff_${version}" "/v/kerbo_${version}" || true; } |
    sed -n "s#^Files /v/ruff_${version}/\(.*\) and .* differ\$#\1#p" | sort >"/v/differ_${version}.txt"
  comm -23 "/v/differ_${version}.txt" "/v/untouched_${version}.txt" >"/v/diff_${version}.txt"
  total="$(find "/v/${version}" -name '*.py' | wc -l)"
  untouched="$(wc -l <"/v/untouched_${version}.txt")"
  echo "${version}: $((total - untouched)) formateados de ${total}; $(wc -l <"/v/diff_${version}.txt") distintos de ruff; ${untouched} que kerbo deja como están"
done
echo "== motivos de lo que queda como está (orig)"
sed -n 's#^kerbo: .*\.py:[0-9]*: \(.*\)$#\1#p' /v/kerbo_orig.log | sort | uniq -c | sort -rn | head -n 40
for version in orig crlf second; do
  echo "== distintos (${version})"
  head -n 40 "/v/diff_${version}.txt"
done
MEASURE
