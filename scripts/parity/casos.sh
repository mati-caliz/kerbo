#!/usr/bin/env bash
# Copia los casos sintéticos de un lenguaje (scripts/parity/<lenguaje>/casos) al corpus como
# casos-<lenguaje>, para medirlos con reference.sh y corpus.sh igual que un repo. Cubren lo que
# ningún repo del corpus usa. Es una copia, nunca un enlace: borrar el corpus no toca el repo.
#
#   scripts/parity/casos.sh rails
set -euo pipefail
language="$1"
here="$(cd "$(dirname "$0")" && pwd)"
parity="$(cd "${KERBO_PARITY:-$here/../../.parity}" && pwd)"
target="$parity/corpus/casos-$language"
rm -rf "$target"
cp -r "$here/$language/casos" "$target"
echo "$target"
