#!/usr/bin/env bash
# Arma el binario estático de Linux x86_64 (musl, sin dependencias del sistema) y lo deja en
# dist/ empaquetado con su sha256 por release/package.sh, igual que el workflow de release. Corre
# en la misma imagen que el gate, como root para sumar el target de musl, con la memoria acotada.
# Se llama envuelto en build-lock.sh.
#
#   release/build-linux.sh
set -euo pipefail

readonly TARGET="x86_64-unknown-linux-musl"
readonly MEMORY_LIMIT="1500m"
readonly BUILD_JOBS=2
readonly CACHE_VOLUME="rust-quality-cache"
readonly VERIFY_CGROUP="builds.slice"

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
image="$(docker images --format '{{.Repository}}:{{.Tag}}' quality-rust | head -1)"
[[ -n "${image}" ]] || {
  echo "Falta la imagen quality-rust: correr antes scripts/verify-rust.sh" >&2
  exit 2
}
user_id="$(id -u)"
group_id="$(id -g)"
mkdir -p "${repo_dir}/dist"

docker run --rm -i -m "${MEMORY_LIMIT}" --cgroup-parent="${VERIFY_CGROUP}" \
  -v "${repo_dir}:/src:ro" -v "${repo_dir}/dist:/dist" -w /src \
  -v "${CACHE_VOLUME}:/cache" \
  -e CARGO_HOME=/cache/cargo -e CARGO_TARGET_DIR=/cache/target/kerbo-release \
  -e CARGO_BUILD_JOBS="${BUILD_JOBS}" -e TARGET="${TARGET}" \
  -e OWNER="${user_id}:${group_id}" \
  "${image}" bash -euo pipefail -s <<'BUILD'
apt-get update -qq && apt-get install -y -qq --no-install-recommends musl-tools >/dev/null
rustup target add "${TARGET}" >/dev/null
cargo build --release --locked --target "${TARGET}"
archive="$(bash release/package.sh "${TARGET}" "${CARGO_TARGET_DIR}/${TARGET}/release/kerbo" /dist)"
chown "${OWNER}" "/dist/${archive}" "/dist/${archive}.sha256"
echo "dist/${archive}"
BUILD
