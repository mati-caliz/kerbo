#!/bin/sh
# Corre adentro de koalaman/shellcheck-alpine: /c es el corpus, /o la salida con files.txt.
set -u
tr -d '\r' < /o/files.txt > /tmp/files.txt
cd /c
if [ "${1:-}" = "--no-directives" ]; then
  rm -rf /tmp/c && mkdir /tmp/c
  while read -r file; do
    mkdir -p "/tmp/c/$(dirname "$file")"
    sed '/^[[:space:]]*#[[:space:]]*shellcheck[[:space:]]/d' "$file" > "/tmp/c/$file"
  done < /tmp/files.txt
  cd /tmp/c
fi
tr '\n' '\0' < /tmp/files.txt | xargs -0 -n 200 shellcheck --norc -f json1 2>/dev/null > /o/shellcheck.raw
exit 0
