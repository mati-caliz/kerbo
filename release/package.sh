#!/usr/bin/env bash
# Empaqueta un binario de kerbo ya compilado como lo publica un release: el binario, el README y
# la licencia en kerbo-<versión>-<target>.tar.gz (.zip para Windows), con su .sha256 al lado en el
# formato de `sha256sum -c`. Lo usan build-linux.sh y el workflow de release, en Linux, macOS y
# el bash de Git en Windows. Imprime el nombre del archivo armado.
#
#   release/package.sh <target> <binario> <carpeta de salida>
set -euo pipefail

readonly WINDOWS_TARGET_MARKER="-windows-"

target="${1:?uso: package.sh <target> <binario> <carpeta de salida>}"
binary="${2:?uso: package.sh <target> <binario> <carpeta de salida>}"
output_dir="${3:?uso: package.sh <target> <binario> <carpeta de salida>}"
repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
version="$(bash "${repo_dir}/release/version.sh")"

sha256_line() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1"
  else
    shasum -a 256 "$1"
  fi
}

staging="$(mktemp -d)"
trap 'rm -rf "${staging}"' EXIT
cp "${binary}" "${repo_dir}/README.md" "${repo_dir}/LICENSE" "${staging}/"
binary_name="$(basename "${binary}")"
mkdir -p "${output_dir}"
output_dir="$(cd "${output_dir}" && pwd)"

if [[ "${target}" == *"${WINDOWS_TARGET_MARKER}"* ]]; then
  archive="kerbo-${version}-${target}.zip"
  if command -v 7z >/dev/null 2>&1; then
    (cd "${staging}" && 7z a -tzip -bso0 -bsp0 "${output_dir}/${archive}" "${binary_name}" README.md LICENSE)
  else
    windows_archive="$(cygpath -w "${output_dir}/${archive}")"
    (cd "${staging}" && powershell -NoProfile -Command \
      "Compress-Archive -Path '${binary_name}','README.md','LICENSE' -DestinationPath '${windows_archive}'")
  fi
else
  archive="kerbo-${version}-${target}.tar.gz"
  tar -czf "${output_dir}/${archive}" -C "${staging}" "${binary_name}" README.md LICENSE
fi

(cd "${output_dir}" && sha256_line "${archive}" >"${archive}.sha256")
echo "${archive}"
