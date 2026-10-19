#!/usr/bin/env bash
# Arma el binario estático de Linux x86_64 (musl, sin dependencias del sistema) y lo deja en
# dist/ empaquetado con su sha256, listo para subir a un release de GitHub. Corre en la misma
# imagen que el gate, como root para sumar el target de musl, con la memoria acotada. Se llama
# envuelto en build-lock.sh.
#
#   release/build-linux.sh
set -euo pipefail

readonly TARGET="x86_64-unknown-linux-musl"
readonly MEMORY_LIMIT="1500m"
readonly BUILD_JOBS=2
readonly CACHE_VOLUME="rust-quality-cache"
readonly VERIFY_CGROUP="builds.slice"

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
version="$(sed -n 's/^version = "\(.*\)"$/\1/p' "${repo_dir}/Cargo.toml" | head -1)"
image="$(docker images --format '{{.Repository}}:{{.Tag}}' quality-rust | head -1)"
[[ -n "${image}" ]] || {
  echo "Falta la imagen quality-rust: correr antes scripts/verify-rust.sh" >&2
  exit 2
}
archive="kerbo-${version}-${TARGET}.tar.gz"
user_id="$(id -u)"
group_id="$(id -g)"
mkdir -p "${repo_dir}/dist"

docker run --rm -i -m "${MEMORY_LIMIT}" --cgroup-parent="${VERIFY_CGROUP}" \
  -v "${repo_dir}:/src:ro" -v "${repo_dir}/dist:/dist" -w /src \
  -v "${CACHE_VOLUME}:/cache" \
  -e CARGO_HOME=/cache/cargo -e CARGO_TARGET_DIR=/cache/target/kerbo-release \
  -e CARGO_BUILD_JOBS="${BUILD_JOBS}" -e TARGET="${TARGET}" -e ARCHIVE="${archive}" \
  -e OWNER="${user_id}:${group_id}" \
  "${image}" bash -euo pipefail -s <<'BUILD'
apt-get update -qq && apt-get install -y -qq --no-install-recommends musl-tools >/dev/null
rustup target add "${TARGET}" >/dev/null
cargo build --release --locked --target "${TARGET}"
staging="$(mktemp -d)"
cp "${CARGO_TARGET_DIR}/${TARGET}/release/kerbo" README.md LICENSE "${staging}/"
tar -czf "/dist/${ARCHIVE}" -C "${staging}" kerbo README.md LICENSE
cd /dist && sha256sum "${ARCHIVE}" >"${ARCHIVE}.sha256"
chown "${OWNER}" "/dist/${ARCHIVE}" "/dist/${ARCHIVE}.sha256"
BUILD
echo "dist/${archive}"
