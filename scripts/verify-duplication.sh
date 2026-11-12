#!/usr/bin/env bash
# Estándar de calidad: lo copia quality/sync.sh y no se edita en el repo.
# Busca código duplicado con jscpd según .jscpd.json, que arma sync.sh. Falla ante un clon que no
# esté en .jscpd-baseline.json y muestra sólo esos; la línea base sólo se achica, y sync.sh --check
# lo verifica contra el último commit.
set -euo pipefail
readonly JSCPD_VERSION="5.4.3"
readonly BASELINE_FILE=".jscpd-baseline.json"
cd "$(dirname "$0")/.."

set +e
report="$(pnpm dlx "jscpd@${JSCPD_VERSION}" . --baseline "${BASELINE_FILE}" --fail-on-new-clones \
  --reporters console 2>&1)"
status=$?
set -e
if ((status != 0)); then
  printf '%s\n' "${report}" | sed 's/\x1b\[[0-9;]*m//g' |
    awk '/^Clone found/ { show = /\[NEW\]/ } show && (/^Clone found/ || /^ /) { print } /^(Found|ERROR|Error)/ { print }'
  echo "Código duplicado nuevo: llevá lo común a un módulo que usen los dos lados." >&2
fi
exit "${status}"
