#!/usr/bin/env bash
# Deja una carpeta de corpus copiada en un volumen de Docker y escribe el nombre del volumen. En
# Windows, leer miles de archivos desde una carpeta montada es lo más lento de Docker Desktop; desde
# un volumen, no. La copia se rehace sólo si cambió el commit de algún repo de la carpeta. Dos
# sesiones a la vez se turnan con una carpeta de lock al lado del corpus: si copiaran juntas, el
# borrado de una se come lo que copia la otra. Cada carril (carpeta `kerbo-<carril>`) usa su propio
# volumen, `<nombre>-<carril>` (`principal` para la carpeta `kerbo`): si compartieran uno, la copia de un carril lo vaciaba en medio de la
# medición del otro y las herramientas de referencia no encontraban los archivos.
#
#   volume="$(scripts/parity/corpus_volume.sh .parity/corpus kerbo-corpus)"
set -euo pipefail

readonly HELPER_IMAGE="alpine:3.22"
readonly LOCK_POLL_SECONDS=5
readonly STALE_LOCK_MINUTES=60
folder="$(cd "$1" && pwd)"
repo_name="$(basename "$(cd "$(dirname "$0")/../.." && pwd)")"
lane="${repo_name#kerbo-}"
[ "$lane" = "$repo_name" ] && lane="principal"
volume="$2-${lane}"
lock="$folder.$volume.lock"
until mkdir "$lock" 2>/dev/null; do
  if [ -n "$(find "$lock" -maxdepth 0 -mmin +"$STALE_LOCK_MINUTES" 2>/dev/null)" ]; then
    rmdir "$lock" 2>/dev/null || true
  fi
  sleep "$LOCK_POLL_SECONDS"
done
trap 'rmdir "$lock" 2>/dev/null || true' EXIT
key="$(for repo in "$folder"/*/; do git -C "$repo" rev-parse HEAD 2>/dev/null || basename "$repo"; done |
  sha256sum | cut -c1-16)"
stored="$(MSYS_NO_PATHCONV=1 docker run --rm -v "$volume:/to" "$HELPER_IMAGE" cat /to/.clave 2>/dev/null || true)"
if [ "$stored" != "$key" ]; then
  echo "copiando $folder al volumen $volume" >&2
  MSYS_NO_PATHCONV=1 docker run --rm --cpus=2 -v "$(cd "$folder" && pwd -W):/from:ro" -v "$volume:/to" \
    -e KEY="$key" "$HELPER_IMAGE" sh -c 'find /to -mindepth 1 -delete && cp -a /from/. /to/ 2>/dev/null; echo "$KEY" > /to/.clave'
fi
echo "$volume"
