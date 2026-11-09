#!/usr/bin/env bash
# Gate de calidad de kerbo: el estándar de infra/quality en el crate (Rust) y los scripts. Va
# envuelto en build-lock.sh. Los .sh de src/ y de testdata/ son fixtures del analizador: tienen los errores a
# propósito y no entran.
#
#   scripts/verify.sh            todo
#   scripts/verify.sh rust|shell
set -euo pipefail

case "${1:-all}" in
  rust | shell | all) ;;
  *)
    printf 'Uso: %s [rust|shell]\n' "$0" >&2
    exit 2
    ;;
esac
cd "$(dirname "$0")/.."

readonly TARGET="${1:-all}"
readonly SHELLCHECK_IMAGE="koalaman/shellcheck:stable"
readonly SHELLCHECK_MEMORY_LIMIT="768m"
readonly VERIFY_CGROUP="builds.slice"

check_rust() {
  bash scripts/verify-rust.sh
}

check_shell() {
  local tracked_scripts
  tracked_scripts="$(git ls-files '*.sh' ':!:src/**' ':!:**/testdata/**')"
  local -a shell_scripts
  mapfile -t shell_scripts <<<"${tracked_scripts}"
  docker run --rm -m "${SHELLCHECK_MEMORY_LIMIT}" --cgroup-parent="${VERIFY_CGROUP}" \
    -v "${PWD}:/mnt:ro" -w /mnt "${SHELLCHECK_IMAGE}" "${shell_scripts[@]}"
}

bash ../infra/quality/sync.sh . --check

case "${TARGET}" in
  rust) check_rust ;;
  shell) check_shell ;;
  all)
    check_rust
    check_shell
    ;;
  *) exit 2 ;;
esac
