#!/usr/bin/env bash
# Deja una carpeta de corpus copiada en un volumen de Docker y escribe el nombre del volumen. En
# Windows, leer miles de archivos desde una carpeta montada es lo más lento de Docker Desktop; desde
# un volumen, no. La copia se rehace sólo si cambió el commit de algún repo de la carpeta.
#
#   volume="$(scripts/parity/corpus_volume.sh .parity/corpus kerbo-corpus)"
set -euo pipefail

readonly HELPER_IMAGE="alpine:3.22"
folder="$(cd "$1" && pwd)"
volume="$2"
key="$(for repo in "$folder"/*/; do git -C "$repo" rev-parse HEAD 2>/dev/null || basename "$repo"; done |
  sha256sum | cut -c1-16)"
stored="$(MSYS_NO_PATHCONV=1 docker run --rm -v "$volume:/to" "$HELPER_IMAGE" cat /to/.clave 2>/dev/null || true)"
if [ "$stored" != "$key" ]; then
  echo "copiando $folder al volumen $volume" >&2
  MSYS_NO_PATHCONV=1 docker run --rm --cpus=2 -v "$(cd "$folder" && pwd -W):/from:ro" -v "$volume:/to" \
    -e KEY="$key" "$HELPER_IMAGE" sh -c 'find /to -mindepth 1 -delete && cp -a /from/. /to/ 2>/dev/null; echo "$KEY" > /to/.clave'
fi
echo "$volume"
