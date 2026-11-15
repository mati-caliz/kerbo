#!/usr/bin/env bash
# Mide todo el corpus Java de una vez: corre kerbo en cada repo de classpaths.txt (imitando a
# Checkstyle y a PMD) y lo compara con las salidas ya guardadas de Checkstyle, PMD y Qodana. No
# vuelve a correr las herramientas de referencia: para eso, checkstyle.py sin --skip-checkstyle,
# pmd.py sin --skip-pmd y qodana.sh. Con SKIP_KERBO=1 tampoco corre kerbo. Deja todo en
# .parity/qodana/measure-all.log.
#
#   python infra/local-bin/heavy_lock.py -- bash scripts/parity/java/measure_all.sh
set -uo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
scripts="$(cd "$here" && { pwd -W 2>/dev/null || pwd; })"
parity="${KERBO_PARITY:-$(cd "$here/../../.." && pwd)/.parity}"
log="$parity/qodana/measure-all.log"
: > "$log"
while read -r repo jars; do
  [[ -z "$repo" ]] && continue
  {
    echo "######## $repo"
    if [[ -z "${SKIP_KERBO:-}" ]]; then
      "$here/kerbo.sh" "$repo" $jars 2>&1 | grep -v '^classpath'
      JAVA_REFERENCE=pmd "$here/kerbo.sh" "$repo" $jars 2>&1 | grep -v '^classpath'
    fi
    echo "== Checkstyle"
    python "$scripts/checkstyle.py" "$repo" --skip-checkstyle
    if [[ -f "$parity/pmd/$repo.json" ]]; then
      echo "== PMD"
      python "$scripts/pmd.py" "$repo" --skip-pmd
    fi
    for results in "$parity/qodana/$repo"/results-*; do
      name="$(basename "$results")"
      case "$name" in
        results-UNUSED_IMPORT | results-SequencedCollectionMethodCanBeUsed)
          echo "== Qodana $name"
          python "$scripts/qodana_rules.py" "$repo" "$name"
          ;;
      esac
    done
    echo "== Qodana, inspecciones de flujo"
    python "$scripts/compare_all.py" "$repo"
  } >> "$log" 2>&1
done < "$here/classpaths.txt"
grep -E '^(########|==|[a-z0-9-]+( imitando a [A-Za-z]+)?: )' "$log" | grep -v -E ': (Checkstyle|PMD) [0-9]+, igual$|Qodana 0, faltan 0, sobran 0$'
