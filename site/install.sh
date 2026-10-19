#!/bin/sh
# Instalador de kerbo: baja la última versión de los releases de GitHub, verifica el sha256 y
# deja el binario en ~/.local/bin (o en KERBO_INSTALL_DIR).
#
#   curl -fsSL https://kerbo.matiascaliz.com.ar/install.sh | sh
set -eu

REPOSITORY="mati-caliz/kerbo"
SUPPORTED_TARGET="x86_64-unknown-linux-musl"
INSTALL_DIR="${KERBO_INSTALL_DIR:-${HOME}/.local/bin}"

fail() {
  echo "kerbo: $1" >&2
  exit 1
}

detect_target() {
  system="$(uname -s)"
  machine="$(uname -m)"
  if [ "${system}" = "Linux" ] && [ "${machine}" = "x86_64" ]; then
    echo "${SUPPORTED_TARGET}"
    return
  fi
  fail "todavía no hay binario para ${system} ${machine}. Con Rust instalado:
  cargo install --git https://github.com/${REPOSITORY} --locked"
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
  else
    echo "kerbo: no hay sha256sum, se instala sin verificar" >&2
  fi
}

command -v curl >/dev/null 2>&1 || fail "hace falta curl"
target="$(detect_target)"
version="${KERBO_VERSION:-$(latest_version)}"
archive="kerbo-${version}-${target}.tar.gz"
base_url="https://github.com/${REPOSITORY}/releases/download/v${version}"
workdir="$(mktemp -d)"
trap 'rm -rf "${workdir}"' EXIT

echo "kerbo: bajando ${version} para ${target}"
curl -fsSL "${base_url}/${archive}" -o "${workdir}/${archive}"
curl -fsSL "${base_url}/${archive}.sha256" -o "${workdir}/${archive}.sha256"
verify_checksum "${workdir}" "${archive}"
tar -xzf "${workdir}/${archive}" -C "${workdir}" kerbo
mkdir -p "${INSTALL_DIR}"
mv "${workdir}/kerbo" "${INSTALL_DIR}/kerbo"
chmod 755 "${INSTALL_DIR}/kerbo"

echo "kerbo: instalado en ${INSTALL_DIR}/kerbo"
case ":${PATH}:" in
  *":${INSTALL_DIR}:"*) ;;
  *) echo "kerbo: agregá ${INSTALL_DIR} al PATH para usarlo desde cualquier carpeta" ;;
esac
