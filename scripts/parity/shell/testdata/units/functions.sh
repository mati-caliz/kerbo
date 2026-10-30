#!/bin/bash
foo() { echo "$1"; }
foo
bar() { echo "${1:-x}"; }
bar
baz() { set -- a; echo "$1"; }
baz
qux() { echo "$@"; }
qux a
qux
# shellcheck disable=SC2120
quux() { echo "$#"; }
quux
