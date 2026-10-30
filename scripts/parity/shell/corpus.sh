#!/usr/bin/env bash
# Clona (o deja como está) el corpus de shell de la paridad contra ShellCheck en
# kerbo-parity/shell-corpus: kubernetes y moby sólo con sus carpetas de scripts, y los demás enteros.
set -euo pipefail
corpus="${SHELL_CORPUS:-$(cd "$(dirname "$0")/../../../.." && pwd)/kerbo-parity/shell-corpus}"
mkdir -p "$corpus"
clone() {
  local name="$1" url="$2"; shift 2
  [ -d "$corpus/$name" ] && return
  if [ "$#" -gt 0 ]; then
    git -c core.autocrlf=false -c core.eol=lf clone -q --depth 1 --filter=blob:none --sparse "$url" "$corpus/$name"
    git -C "$corpus/$name" sparse-checkout set "$@"
  else
    git -c core.autocrlf=false -c core.eol=lf clone -q --depth 1 "$url" "$corpus/$name"
  fi
}
clone kubernetes https://github.com/kubernetes/kubernetes hack cluster build test
clone moby https://github.com/moby/moby hack contrib
clone nvm https://github.com/nvm-sh/nvm
clone rustup https://github.com/rust-lang/rustup
clone ohmyzsh https://github.com/ohmyzsh/ohmyzsh
clone docker-install https://github.com/docker/docker-install
clone acme.sh https://github.com/acmesh-official/acme.sh
clone bash-it https://github.com/Bash-it/bash-it
clone dokku https://github.com/dokku/dokku
clone neofetch https://github.com/dylanaraps/neofetch
echo "$corpus"
