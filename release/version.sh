#!/usr/bin/env bash
# Imprime la versión del crate, leída del Cargo.toml. Es la que llevan los nombres de los
# archivos del release, la fórmula de Homebrew y el tag `v<versión>` que dispara el workflow.
#
#   release/version.sh
set -euo pipefail

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
version="$(sed -n 's/^version = "\(.*\)"$/\1/p' "${repo_dir}/Cargo.toml" | head -1)"
[[ -n "${version}" ]] || {
  echo "No se encontró la versión en ${repo_dir}/Cargo.toml" >&2
  exit 2
}
echo "${version}"
