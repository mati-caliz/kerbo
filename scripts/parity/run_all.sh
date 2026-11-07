#!/usr/bin/env bash
# Corre los corpus de paridad de cada familia, uno detrás del otro y cada paso por la cola de
# heavy_lock.py, con las versiones de versions.toml. Deja la salida entera de cada familia en
# .parity/logs/<fecha>/<familia>.txt y, al final, el resumen de cada una: lo que cuesta subir de
# versión una herramienta es la diferencia entre dos corridas.
#
#   scripts/parity/run_all.sh                 # todas las familias
#   scripts/parity/run_all.sh python shell    # sólo esas
#   QODANA=1 scripts/parity/run_all.sh java   # también vuelve a correr Qodana (tarda mucho)
#
# HEAVY_LOCK apunta a heavy_lock.py (por defecto, el de dotfiles al lado del repo). Si la corrida
# entera ya va adentro de heavy_lock.py, PARITY_QUEUED=1 hace que los pasos no pidan otro lugar.
set -euo pipefail

readonly FAMILIES=(python ecmascript shell go java)
readonly JAVA_REPOS=(junit4 commons-lang)
readonly JAVA_INSPECTIONS=(RedundantThrows AutoCloseableResource DataFlowIssue)
readonly WAIT_SECONDS=3000
here="$(cd "$(dirname "$0")" && { pwd -W 2>/dev/null || pwd; })"
repo="$(cd "$here/../.." && { pwd -W 2>/dev/null || pwd; })"
heavy_lock="${HEAVY_LOCK:-$(cd "$repo/.." && pwd)/dotfiles/local-bin/heavy_lock.py}"
logs="$repo/.parity/logs/$(date +%Y-%m-%d_%H%M)"
mkdir -p "$logs"

queued() {
  if [ -n "${PARITY_QUEUED:-}" ]; then
    bash -c "$1"
  else
    MSYS_NO_PATHCONV=1 python "$heavy_lock" --wait "$WAIT_SECONDS" -- bash -c "$1"
  fi
}

eslint_rules() {
  python - "$repo/src/rules/families/eslint.rs" <<'PY'
import re, sys
print(",".join(re.findall(r'=> "([^"]+)"', open(sys.argv[1], encoding="utf-8").read())))
PY
}

run_family() {
  case "$1" in
    python) queued "'$here/python/corpus.sh'" ;;
    ecmascript) queued "'$here/ecmascript/corpus.sh' '$(eslint_rules)'" ;;
    shell) queued "'$here/shell/shellcheck.sh' && '$here/shell/kerbo.sh' && python '$here/shell/compare.py'" ;;
    go) queued "'$here/go/helper-build.sh' && '$here/go/corpus-golangci.sh' && '$here/go/corpus-helper.sh' && python '$here/go/compare_corpus.py'" ;;
    java)
      for name in "${JAVA_REPOS[@]}"; do
        if [ -n "${QODANA:-}" ]; then
          for inspection in "${JAVA_INSPECTIONS[@]}"; do queued "'$here/java/qodana.sh' $name $inspection"; done
        fi
      done
      while read -r name jars; do
        queued "'$here/java/kerbo.sh' $name '$jars'"
      done < "$here/java/classpaths.txt"
      for name in "${JAVA_REPOS[@]}"; do python "$here/java/compare_all.py" "$name"; done
      ;;
    *) echo "familia desconocida: $1" >&2; return 1 ;;
  esac
}

families=("$@")
[ "${#families[@]}" -eq 0 ] && families=("${FAMILIES[@]}")
for family in "${families[@]}"; do
  echo "== $family"
  run_family "$family" > "$logs/$family.txt" 2>&1 || echo "  falló: ver $logs/$family.txt"
  tail -5 "$logs/$family.txt" | sed 's/^/  /'
done
echo
echo "Sin arnés en el repo todavía: migraciones (squawk), Dockerfile (hadolint), secretos (gitleaks) y"
echo "dependencias (osv-scanner). Salida completa en $logs"
