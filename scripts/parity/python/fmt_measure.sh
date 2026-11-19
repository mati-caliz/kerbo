#!/usr/bin/env bash
# Mide `kerbo fmt` de Python contra `ruff format` sobre los `.py` del corpus de Python, en el
# volumen `fmt-parity-python`: `/v/orig` (los archivos como están), `/v/ruff_orig` (lo que deja
# `ruff format --isolated`, con sus errores en `/v/ruff_orig.err`) y `/v/kerbo_orig` (lo que deja
# kerbo, con su log en `/v/kerbo_orig.log`). Mientras el formateador esté incompleto, un archivo con
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
rm -rf /v/kerbo_orig
cp -a /v/orig /v/kerbo_orig
(cd /v/kerbo_orig && "${kerbo}" fmt --python . >/v/kerbo_orig.log 2>&1) || true
sed -n 's#^kerbo: \(.*\.py\):[0-9]*: .*quedó como estaba$#\1#p' /v/kerbo_orig.log | sed 's#^\./##' | sort -u >/v/untouched_orig.txt
{ diff -rq /v/ruff_orig /v/kerbo_orig || true; } |
  sed -n 's#^Files /v/ruff_orig/\(.*\) and .* differ$#\1#p' | sort >/v/differ_orig.txt
comm -23 /v/differ_orig.txt /v/untouched_orig.txt >/v/diff_orig.txt
total="$(find /v/orig -name '*.py' | wc -l)"
untouched="$(wc -l </v/untouched_orig.txt)"
echo "orig: $((total - untouched)) formateados de ${total}; $(wc -l </v/diff_orig.txt) distintos de ruff; ${untouched} que kerbo deja como están"
echo "== motivos de lo que queda como está"
sed -n 's#^kerbo: .*\.py:[0-9]*: \(.*\)$#\1#p' /v/kerbo_orig.log | sort | uniq -c | sort -rn | head -n 40
echo "== distintos"
head -n 60 /v/diff_orig.txt
MEASURE
