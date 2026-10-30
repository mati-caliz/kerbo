#!/bin/sh
declare -a x
typeset y
shopt -s nullglob
pushd /tmp
popd
mapfile -t lines < f
compgen -W a
complete -F f g
builtin echo hi
caller
readarray z
disown
suspend
enable -n echo
dirs
