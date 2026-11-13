#!/usr/bin/env bash
# Estándar de calidad: lo copia quality/sync.sh y no se edita en el repo.
# Busca código duplicado con jscpd según .jscpd.json, que arma sync.sh. Cuenta clones y no huellas:
# la huella incluye el archivo, así que mover código que ya estaba duplicado no es deuda nueva. Falla
# si hay más clones que en .jscpd-baseline.json (y muestra los que no estaban) o si hay menos sin
# haber achicado la línea base. sync.sh --check verifica que la línea base no crezca.
set -euo pipefail
readonly JSCPD_VERSION="5.4.3"
readonly BASELINE_FILE=".jscpd-baseline.json"
cd "$(dirname "$0")/.."

report="$(pnpm dlx "jscpd@${JSCPD_VERSION}" . --baseline "${BASELINE_FILE}" --reporters console 2>&1 |
  sed 's/\x1b\[[0-9;]*m//g')"
clone_count="$(printf '%s\n' "${report}" | sed -n 's/^Found \([0-9]*\) clones.*/\1/p' | tail -1)"
baseline_count="$(python3 -c 'import json, sys; print(sum(json.load(open(sys.argv[1]))["fingerprints"].values()))' \
  "${BASELINE_FILE}")"
clone_count="${clone_count:-0}"
if ((clone_count > baseline_count)); then
  printf '%s\n' "${report}" |
    awk '/^Clone found/ { show = /\[NEW\]/ } show && (/^Clone found/ || /^ /) { print }'
  echo "Código duplicado nuevo: ${clone_count} clones contra ${baseline_count} de la línea base." >&2
  echo "Llevá lo común a un módulo que usen los dos lados." >&2
  exit 1
fi
if ((clone_count < baseline_count)); then
  echo "Bajaste de ${baseline_count} a ${clone_count} clones: actualizá la línea base con" >&2
  echo "  pnpm dlx jscpd@${JSCPD_VERSION} . --baseline ${BASELINE_FILE} --update-baseline" >&2
  exit 1
fi
echo "jscpd: ${clone_count} clones, los mismos de la línea base"
