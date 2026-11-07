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
# HEAVY_LOCK apunta a heavy_lock.py (por defecto, el de infra al lado del repo). Si la corrida
# entera ya va adentro de heavy_lock.py, PARITY_QUEUED=1 hace que los pasos no pidan otro lugar.
set -euo pipefail

readonly FAMILIES=(python ecmascript shell go java)
readonly JAVA_REPOS=(junit4 commons-lang)
readonly JAVA_INSPECTIONS=(RedundantThrows AutoCloseableResource DataFlowIssue)
readonly WAIT_SECONDS=3000
here="$(cd "$(dirname "$0")" && { pwd -W 2>/dev/null || pwd; })"
repo="$(cd "$here/../.." && { pwd -W 2>/dev/null || pwd; })"
heavy_lock="${HEAVY_LOCK:-$(cd "$repo/.." && pwd)/infra/local-bin/heavy_lock.py}"
logs="${KERBO_PARITY:-$repo/.parity}/logs/$(date +%Y-%m-%d_%H%M)"
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

# La salida de una herramienta de referencia sólo cambia si cambia su versión, el corpus o lo que se
# le pide: se guarda al lado la clave con que se sacó, y con la misma clave no se vuelve a correr.
version_of() { python "$here/versions.py" version "$1"; }
fingerprint() { printf '%s' "$*" | sha256sum | cut -c1-16; }
corpus_key() { git -C "$repo" hash-object "$here/corpus.toml"; }
shell_corpus_key() {
  local corpus="${KERBO_PARITY:-$repo/.parity}/shell-corpus" folder
  for folder in "$corpus"/*/; do git -C "$folder" rev-parse HEAD 2>/dev/null || echo "$folder"; done
}
is_fresh() { [ -f "$1.clave" ] && [ "$(cat "$1.clave")" = "$2" ]; }
remember() { printf '%s' "$2" > "$1.clave"; }

reference() {
  local output="$1" key="$2" refresh="$3" reuse="$4"
  if is_fresh "$output" "$key"; then
    echo "referencia al día ($(basename "$output")): no se vuelve a correr"
    queued "$reuse"
  else
    queued "$refresh" && remember "$output" "$key"
  fi
}

run_family() {
  local parity="${KERBO_PARITY:-$repo/.parity}" key rules
  case "$1" in
    python)
      key="$(fingerprint "$(version_of ruff)" "$(corpus_key)" "$(python "$here/python/compare.py" --select-codes)")"
      reference "$here/python/out/ruff.json" "$key" "'$here/python/corpus.sh'" "'$here/python/corpus.sh' --skip-ruff"
      ;;
    ecmascript)
      rules="$(eslint_rules)"
      key="$(fingerprint "$(version_of eslint)" "$(version_of typescript-eslint)" "$(corpus_key)" "$rules")"
      reference "$parity/eslint/out_rules.json" "$key" "'$here/ecmascript/corpus.sh' '$rules'" \
        "'$here/ecmascript/corpus.sh' '$rules' --skip-eslint"
      ;;
    shell)
      key="$(fingerprint "$(version_of shellcheck)" "$(shell_corpus_key)")"
      reference "$here/shell/out/shellcheck.json" "$key" \
        "'$here/shell/shellcheck.sh' && '$here/shell/kerbo.sh' && python '$here/shell/compare.py'" \
        "'$here/shell/kerbo.sh' && python '$here/shell/compare.py'"
      ;;
    go)
      key="$(fingerprint "$(version_of golangci-lint)" "$(git -C "$repo" hash-object "$here/go/golangci-default.yml")")"
      reference "$here/go/out/default.json" "$key" \
        "'$here/go/helper-build.sh' && '$here/go/corpus-golangci.sh' && '$here/go/corpus-helper.sh' && python '$here/go/compare_corpus.py'" \
        "'$here/go/helper-build.sh' && '$here/go/corpus-helper.sh' && python '$here/go/compare_corpus.py'"
      ;;
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
