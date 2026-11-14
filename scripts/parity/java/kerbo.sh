#!/usr/bin/env bash
# Compila kerbo con el perfil parity (release sin LTO) y lo corre sobre un repo Java del corpus con un JDK real (el volumen jdk21)
# y los jars indicados (admite comodines), sacados de la caché de Qodana de ese repo. Deja
# .parity/qodana/<repo>/kerbo.json.
#
#   scripts/parity/java/kerbo.sh junit4 hamcrest-core-1.3.jar
#
# Los jars de cada repo del corpus están en classpaths.txt. Con "*.jar" entra el propio jar del repo,
# que tapa a sus clases y ensucia la comparación. Si existe configs/<repo>.kerbo.toml, se copia como
# kerbo.toml del repo, para dejar afuera lo que Qodana no analiza. Con JAVA_REFERENCE=pmd se le suma
# `[java] reference = "pmd"` y la salida queda en kerbo-pmd.json.
set -euo pipefail

readonly CPUS=4
readonly BUILD_JOBS=4
readonly MEMORY_LIMIT="8g"
readonly JDK_VOLUME="jdk21"
repo_name="$1"
shift
jars="$*"
here="$(cd "$(dirname "$0")" && pwd)"
crate="$(cd "$here/../../.." && pwd -W)"
target="$(basename "$crate")"
parity="${KERBO_PARITY:-$(cd "$here/../../.." && pwd)/.parity}"
image="$(docker images --format '{{.Repository}}:{{.Tag}}' quality-rust | head -1)"
mkdir -p "$parity/qodana/$repo_name"
repo_config="$here/configs/$repo_name.kerbo.toml"
corpus_config="$parity/corpus/$repo_name/kerbo.toml"
output="kerbo.json"
rm -f "$corpus_config"
if [[ -f "$repo_config" ]]; then
  cp "$repo_config" "$corpus_config"
fi
if [[ -n "${JAVA_REFERENCE:-}" ]]; then
  printf '\n[java]\nreference = "%s"\n' "$JAVA_REFERENCE" >> "$corpus_config"
  output="kerbo-$JAVA_REFERENCE.json"
fi
MSYS_NO_PATHCONV=1 docker run --rm -m "$MEMORY_LIMIT" --cpus="$CPUS" -v "$crate:/src:ro" -w /src \
  -v rust-quality-cache:/cache -v "$JDK_VOLUME:/jdk:ro" -v "qodana-cache-$repo_name:/m2:ro" \
  -v "$(cd "$parity/corpus/$repo_name" && pwd -W):/c:ro" -v "$(cd "$parity/qodana/$repo_name" && pwd -W):/o" \
  -e CARGO_HOME=/cache/cargo -e CARGO_TARGET_DIR="/cache/target/$target" -e CARGO_BUILD_JOBS="$BUILD_JOBS" \
  -e CARGO_TERM_COLOR=never -e RUSTFLAGS="-C link-arg=-fuse-ld=mold" -e JAVA_HOME=/jdk -e JARS="$jars" -e TARGET="$target" -e OUTPUT="$output" \
  "$image" bash -euo pipefail -c '
    cargo build --profile parity --locked --bin kerbo 2>&1 | tail -1
    classpath=""
    for jar in $JARS; do
      for found in $(find /m2 -name "$jar"); do classpath="$classpath:$found"; done
    done
    export KERBO_JAVA_CLASSPATH="${classpath#:}"
    echo "classpath: $KERBO_JAVA_CLASSPATH"
    cd /c
    start=$(date +%s)
    "/cache/target/$TARGET/parity/kerbo" check --format json --no-cache . > "/o/$OUTPUT" 2> /o/kerbo.log || true
    echo "kerbo tardó $(( $(date +%s) - start )) s"
    tail -2 /o/kerbo.log'
