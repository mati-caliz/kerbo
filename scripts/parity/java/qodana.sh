#!/usr/bin/env bash
# Corre una sola inspección de Qodana (RedundantThrows si no se dice otra) sobre una copia de un repo
# del corpus, y deja el SARIF en kerbo-parity/qodana/<repo>/results-<inspección>. Trabaja en una copia
# porque Qodana escribe en la carpeta del proyecto.
#
#   scripts/parity/java/qodana.sh junit4 AutoCloseableResource
set -euo pipefail

readonly IMAGE="jetbrains/qodana-jvm-community:latest"
readonly CPUS=4
readonly MEMORY_LIMIT="8g"
repo="$1"
inspection="${2:-RedundantThrows}"
scripts_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
parity="$(cd "${scripts_dir}/../../../../kerbo-parity" && pwd)"
work="${parity}/qodana/${repo}"
results="${work}/results-${inspection}"
mkdir -p "${results}"
rm -rf "${work}/project"
cp -r "${parity}/corpus/${repo}" "${work}/project"
cat >"${work}/project/qodana.yaml" <<YAML
version: "1.0"
profile:
  name: empty
include:
  - name: ${inspection}
YAML
MSYS_NO_PATHCONV=1 docker run --rm --cpus="${CPUS}" -m "${MEMORY_LIMIT}" \
  -v "$(cd "${work}/project" && pwd -W):/data/project" \
  -v "$(cd "${results}" && pwd -W):/data/results" \
  -v "qodana-cache-${repo}:/data/cache" \
  "${IMAGE}" --disable-sanity --fail-threshold 100000
