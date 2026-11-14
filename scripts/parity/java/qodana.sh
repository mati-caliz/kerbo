#!/usr/bin/env bash
# Corre una sola inspección de Qodana (RedundantThrows si no se dice otra) sobre una copia de un repo
# del corpus, y deja el SARIF en .parity/qodana/<repo>/results-<inspección>. Trabaja en una copia
# porque Qodana escribe en la carpeta del proyecto. Varias inspecciones separadas por comas corren
# juntas y dejan un solo SARIF, con el nombre de la primera.
#
#   scripts/parity/java/qodana.sh junit4 AutoCloseableResource
#   scripts/parity/java/qodana.sh junit4 UNUSED_IMPORT,UnnecessaryInterfaceModifier
set -euo pipefail

IMAGE="$(python "$(dirname "${BASH_SOURCE[0]}")/../versions.py" image qodana)"
readonly IMAGE
readonly CPUS=4
readonly MEMORY_LIMIT="8g"
repo="$1"
inspection="${2:-RedundantThrows}"
scripts_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
parity="$(cd "${KERBO_PARITY:-${scripts_dir}/../../../.parity}" && pwd)"
work="${parity}/qodana/${repo}"
results="${work}/results-${inspection%%,*}"
mkdir -p "${results}"
rm -rf "${work}/project"
cp -r "${parity}/corpus/${repo}" "${work}/project"
cat >"${work}/project/qodana.yaml" <<YAML
version: "1.0"
profile:
  name: empty
include:
$(tr ',' '\n' <<<"${inspection}" | sed 's/^/  - name: /')
YAML
MSYS_NO_PATHCONV=1 docker run --rm --cpus="${CPUS}" -m "${MEMORY_LIMIT}" \
  -v "$(cd "${work}/project" && pwd -W):/data/project" \
  -v "$(cd "${results}" && pwd -W):/data/results" \
  -v "qodana-cache-${repo}:/data/cache" \
  "${IMAGE}" --disable-sanity --fail-threshold 100000
