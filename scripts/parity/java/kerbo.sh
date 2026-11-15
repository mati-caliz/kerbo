#!/usr/bin/env bash
# Compila kerbo con el perfil parity (release sin LTO) y lo corre sobre un repo Java del corpus con un JDK real (el volumen jdk21)
# y los jars indicados (admite comodines), sacados de la caché de Qodana de ese repo. Deja
# .parity/qodana/<repo>/kerbo.json.
#
#   scripts/parity/java/kerbo.sh junit4 hamcrest-core-1.3.jar
#
# Los jars de cada repo del corpus están en classpaths.txt. Con "*.jar" entra el propio jar del repo,
# que tapa a sus clases y ensucia la comparación. Si existe configs/<repo>.kerbo.toml, se copia como
# kerbo.toml del repo, para dejar afuera lo que Qodana no analiza, con `[java] reference =
# "checkstyle"`. Con JAVA_REFERENCE=pmd la misma
# configuración más `[java] reference = "pmd"` va a kerbo-pmd.toml (aparte, para que otra corrida
# sobre el mismo repo no la pise) y la salida queda en kerbo-pmd.json. KERBO_JDK cambia el volumen o la
# carpeta del JDK (un runtime de jlink, por ejemplo).
set -euo pipefail

readonly CPUS=4
readonly BUILD_JOBS=4
readonly MEMORY_LIMIT="8g"
readonly JDK_VOLUME="${KERBO_JDK:-jdk21}"
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
config_arguments=""
# La referencia va siempre explícita: sin ella kerbo la deduce del build, y un repo con
# maven-pmd-plugin (como Maven) se mediría contra PMD en la corrida de Checkstyle.
{ [[ -f "$repo_config" ]] && cat "$repo_config"; printf '\n[java]\nreference = "checkstyle"\n'; } > "$corpus_config"
if [[ -n "${JAVA_REFERENCE:-}" ]]; then
  reference_config="kerbo-$JAVA_REFERENCE.toml"
  { [[ -f "$repo_config" ]] && cat "$repo_config"; printf '\n[java]\nreference = "%s"\n' "$JAVA_REFERENCE"; } \
    > "$parity/corpus/$repo_name/$reference_config"
  config_arguments="--config ./$reference_config"
  output="kerbo-$JAVA_REFERENCE.json"
fi
MSYS_NO_PATHCONV=1 docker run --rm -m "$MEMORY_LIMIT" --cpus="$CPUS" -v "$crate:/src:ro" -w /src \
  -v rust-quality-cache:/cache -v "$JDK_VOLUME:/jdk:ro" -v "qodana-cache-$repo_name:/m2:ro" \
  -v "$(cd "$parity/corpus/$repo_name" && pwd -W):/c:ro" -v "$(cd "$parity/qodana/$repo_name" && pwd -W):/o" \
  -e CARGO_HOME=/cache/cargo -e CARGO_TARGET_DIR="/cache/target/$target" -e CARGO_BUILD_JOBS="$BUILD_JOBS" \
  -e CARGO_TERM_COLOR=never -e RUSTFLAGS="-C link-arg=-fuse-ld=mold" -e JAVA_HOME=/jdk -e JARS="$jars" -e TARGET="$target" -e OUTPUT="$output" -e CONFIG_ARGUMENTS="$config_arguments" \
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
    "/cache/target/$TARGET/parity/kerbo" check --format json --no-cache $CONFIG_ARGUMENTS . > "/o/$OUTPUT" 2> /o/kerbo.log || true
    echo "kerbo tardó $(( $(date +%s) - start )) s"
    tail -2 /o/kerbo.log'
