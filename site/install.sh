#!/bin/sh
# Instalador de kerbo: baja la última versión de los releases de GitHub para el sistema y la
# arquitectura de la máquina (Linux, macOS y Windows desde el bash de Git, en x86_64 y ARM),
# verifica el sha256 y deja el binario en ~/.local/bin (o en KERBO_INSTALL_DIR).
#
#   curl -fsSL https://kerbo.matiascaliz.com.ar/install.sh | sh
set -eu

REPOSITORY="mati-caliz/kerbo"
LINUX_X86_TARGET="x86_64-unknown-linux-musl"
LINUX_ARM_TARGET="aarch64-unknown-linux-musl"
MACOS_X86_TARGET="x86_64-apple-darwin"
MACOS_ARM_TARGET="aarch64-apple-darwin"
WINDOWS_X86_TARGET="x86_64-pc-windows-msvc"
WINDOWS_ARM_TARGET="aarch64-pc-windows-msvc"
INSTALL_DIR="${KERBO_INSTALL_DIR:-${HOME}/.local/bin}"

fail() {
  echo "kerbo: $1" >&2
  exit 1
}

unsupported() {
  fail "todavía no hay binario para $1 $2. Con Rust instalado:
  cargo install kerbo --locked"
}

macos_target() {
  translated="$(sysctl -n sysctl.proc_translated 2>/dev/null || true)"
  if [ "$1" = "arm64" ] || [ "${translated}" = "1" ]; then
    echo "${MACOS_ARM_TARGET}"
  else
    echo "${MACOS_X86_TARGET}"
  fi
}

detect_target() {
  system="$(uname -s)"
  machine="$(uname -m)"
  case "${system}:${machine}" in
    Linux:x86_64) echo "${LINUX_X86_TARGET}" ;;
    Linux:aarch64 | Linux:arm64) echo "${LINUX_ARM_TARGET}" ;;
    Darwin:arm64 | Darwin:x86_64) macos_target "${machine}" ;;
    MINGW*:x86_64 | MSYS*:x86_64 | CYGWIN*:x86_64) echo "${WINDOWS_X86_TARGET}" ;;
    MINGW*:aarch64 | MSYS*:aarch64 | CYGWIN*:aarch64) echo "${WINDOWS_ARM_TARGET}" ;;
    *) unsupported "${system}" "${machine}" ;;
  esac
}

latest_version() {
  location="$(curl -fsSI "https://github.com/${REPOSITORY}/releases/latest" | tr -d '\r' |
    sed -n 's/^[Ll]ocation: .*\/tag\/v\(.*\)$/\1/p')"
  [ -n "${location}" ] || fail "no se pudo averiguar la última versión"
  echo "${location}"
}

verify_checksum() {
  directory="$1"
  archive="$2"
  if command -v sha256sum >/dev/null 2>&1; then
    (cd "${directory}" && sha256sum -c "${archive}.sha256" >/dev/null) || fail "el sha256 no coincide"
  elif command -v shasum >/dev/null 2>&1; then
    (cd "${directory}" && shasum -a 256 -c "${archive}.sha256" >/dev/null) || fail "el sha256 no coincide"
  else
    echo "kerbo: no hay sha256sum ni shasum, se instala sin verificar" >&2
  fi
}

command -v curl >/dev/null 2>&1 || fail "hace falta curl"
target="$(detect_target)"
version="${KERBO_VERSION:-$(latest_version)}"
case "${target}" in *-windows-*) windows=1 ;; *) windows=0 ;; esac
if [ "${windows}" = 1 ]; then
  command -v unzip >/dev/null 2>&1 || fail "hace falta unzip"
  archive="kerbo-${version}-${target}.zip"
  binary="kerbo.exe"
else
  archive="kerbo-${version}-${target}.tar.gz"
  binary="kerbo"
fi
base_url="https://github.com/${REPOSITORY}/releases/download/v${version}"
workdir="$(mktemp -d)"
trap 'rm -rf "${workdir}"' EXIT

echo "kerbo: bajando ${version} para ${target}"
curl -fsSL "${base_url}/${archive}" -o "${workdir}/${archive}"
curl -fsSL "${base_url}/${archive}.sha256" -o "${workdir}/${archive}.sha256"
verify_checksum "${workdir}" "${archive}"
if [ "${windows}" = 1 ]; then
  unzip -q "${workdir}/${archive}" "${binary}" -d "${workdir}"
else
  tar -xzf "${workdir}/${archive}" -C "${workdir}" "${binary}"
fi
mkdir -p "${INSTALL_DIR}"
mv "${workdir}/${binary}" "${INSTALL_DIR}/${binary}"
chmod 755 "${INSTALL_DIR}/${binary}"

echo "kerbo: instalado en ${INSTALL_DIR}/${binary}"
case ":${PATH}:" in
  *":${INSTALL_DIR}:"*) ;;
  *) echo "kerbo: agregá ${INSTALL_DIR} al PATH para usarlo desde cualquier carpeta" ;;
esac
