#!/usr/bin/env bash
# Pone en la fórmula de Homebrew la versión del Cargo.toml y el sha256 de cada archivo del
# release, leído de los .sha256 que deja release/package.sh. Cada `sha256` toma el hash del target
# que nombra la `url` de arriba, así la fórmula se edita en el lugar y sigue siendo Ruby válido.
# Falla si a algún target de la fórmula le falta su archivo.
#
#   release/homebrew/update-formula.sh <carpeta con los .sha256> [fórmula]
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
checksums_dir="${1:?uso: update-formula.sh <carpeta con los .sha256> [fórmula]}"
formula="${2:-${script_dir}/kerbo.rb}"
version="$(bash "${script_dir}/../version.sh")"
updated="$(mktemp)"
trap 'rm -f "${updated}"' EXIT

awk -v version="${version}" -v checksums_dir="${checksums_dir}" '
  /^  version "/ { sub(/"[^"]*"/, "\"" version "\"") }
  /^ *url "/ {
    match($0, /kerbo-#\{version\}-[^"]*\.tar\.gz/)
    archive = substr($0, RSTART, RLENGTH)
    sub(/#\{version\}/, version, archive)
  }
  /^ *sha256 "/ {
    checksum_file = checksums_dir "/" archive ".sha256"
    if ((getline line < checksum_file) <= 0) {
      print "Falta " checksum_file > "/dev/stderr"
      exit 2
    }
    close(checksum_file)
    split(line, fields, " ")
    sub(/"[^"]*"/, "\"" fields[1] "\"")
  }
  { print }
' "${formula}" >"${updated}"
cp "${updated}" "${formula}"
echo "${formula}: ${version}"
