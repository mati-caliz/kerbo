#!/usr/bin/env bash
# Busca secretos con gitleaks en lo que todavía no llegó al remoto: lo que está en el índice, lo
# modificado fuera de él, los archivos nuevos que git no ignora y los commits sin pushear. Lo copia
# quality/sync.sh como scripts/verify-secrets.sh, no se edita en el repo y lo corre
# `sync.sh --check`, así entra en el gate de todos sin tocarlos. El historial ya pusheado se auditó
# entero una vez por repo; lo nuevo pasa por acá antes de salir. Una excepción va en
# .gitleaksignore (la huella que imprime gitleaks) con el motivo en la línea de arriba.
#
#   scripts/verify-secrets.sh
set -euo pipefail

readonly GITLEAKS_IMAGE="zricethezav/gitleaks:v8.30.1@sha256:c00b6bd0aeb3071cbcb79009cb16a60dd9e0a7c60e2be9ab65d25e6bc8abbb7f"
readonly MEMORY_LIMIT="512m"
readonly VERIFY_CGROUP="builds.slice"
readonly IGNORE_FILE=".gitleaksignore"
readonly UNPUSHED_COMMITS="--branches --not --remotes"

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly repo_dir
work_dir="$(mktemp -d)"
readonly work_dir untracked_dir="${work_dir}/untracked" untracked_list="${work_dir}/untracked.list"
trap 'rm -rf "${work_dir}"' EXIT
mkdir "${untracked_dir}"
host_user_id="$(id -u)"
host_group_id="$(id -g)"
readonly host_user="${host_user_id}:${host_group_id}"

check_ignore_reasons() {
  local previous_line="" line line_number=0 failures=0
  [[ -f "${repo_dir}/${IGNORE_FILE}" ]] || return 0
  while IFS= read -r line; do
    line_number=$((line_number + 1))
    if [[ -n "${line}" && "${line}" != \#* && "${previous_line}" != \#* ]]; then
      echo "${IGNORE_FILE}:${line_number}: la excepción lleva el motivo en un comentario arriba" >&2
      failures=$((failures + 1))
    fi
    [[ -n "${line}" ]] && previous_line="${line}"
  done <"${repo_dir}/${IGNORE_FILE}"
  ((failures == 0))
}

run_gitleaks() {
  local working_directory="$1"
  shift
  docker run --rm -m "${MEMORY_LIMIT}" --cgroup-parent="${VERIFY_CGROUP}" --network none --log-driver=none \
    --user "${host_user}" -e HOME=/tmp \
    -v "${repo_dir}:/repo:ro" -v "${untracked_dir}:/untracked:ro" -w "${working_directory}" \
    "${GITLEAKS_IMAGE}" "$@" --config /repo/.gitleaks.toml --gitleaks-ignore-path /repo \
    --no-banner --redact --log-level warn --verbose
}

check_ignore_reasons
run_gitleaks /repo git --staged --pre-commit .
run_gitleaks /repo git --pre-commit .
git -C "${repo_dir}" ls-files -z --others --exclude-standard >"${untracked_list}"
if [[ -s "${untracked_list}" ]]; then
  tar -C "${repo_dir}" --null --files-from "${untracked_list}" -cf - | tar -C "${untracked_dir}" -xf -
  run_gitleaks /untracked dir .
fi
run_gitleaks /repo git --log-opts="${UNPUSHED_COMMITS}" .
echo "gitleaks: sin secretos en lo que todavía no está en el remoto"
