#!/usr/bin/env bash
# Falla si la cobertura queda debajo del estándar, en total o en algún archivo.
#
# Lee el LCOV de cargo-llvm-cov. La cobertura de ramas de Rust todavía es inestable (sólo en
# nightly), así que se exige la de líneas, como en Go: el umbral de líneas de
# quality/coverage.conf al total y el piso por archivo a cada uno, para que un archivo sin tests
# no se esconda detrás del promedio. Lo genera quality/sync.sh con los umbrales: no se edita en
# el repo.
#
#   scripts/check-rust-coverage.sh coverage.lcov
set -euo pipefail

readonly MIN_TOTAL=80
readonly MIN_PER_FILE=50

report="${1:?uso: check-rust-coverage.sh <coverage.lcov>}"
[[ -s "${report}" ]] || {
  echo "No existe o está vacío: ${report}" >&2
  exit 2
}

awk -v min_total="${MIN_TOTAL}" -v min_file="${MIN_PER_FILE}" '
  /^SF:/ { file = substr($0, 4) }
  /^LF:/ { file_total[file] += substr($0, 4); total += substr($0, 4) }
  /^LH:/ { file_covered[file] += substr($0, 4); total_covered += substr($0, 4) }
  END {
    if (total == 0) {
      print "El reporte no tiene líneas medidas" > "/dev/stderr"
      exit 2
    }
    failed = 0
    for (file in file_total) {
      if (file_total[file] == 0) continue
      percent = 100 * file_covered[file] / file_total[file]
      if (percent < min_file) {
        printf "%s: %.1f %% de líneas (piso %d %%)\n", file, percent, min_file > "/dev/stderr"
        failed = 1
      }
    }
    percent = 100 * total_covered / total
    printf "Cobertura de líneas: %.1f %% (mínimo %d %%)\n", percent, min_total
    if (percent < min_total) failed = 1
    exit failed
  }
' "${report}"
