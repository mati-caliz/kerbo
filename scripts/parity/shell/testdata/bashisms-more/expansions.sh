#!/bin/sh
echo ${x/a/b} ${x//a} ${x:1} ${x: -1} ${x:-d} ${x:=d} ${x:+d} ${x:?d} ${#x} ${x#a} ${x%%b}
echo ${!p} ${x^} ${x,,} ${@:2} ${*%a} ${#@} ${#*}
echo ${a[0]} ${a[*]} ${!a[*]} ${!pre@}
echo $BASH_SOURCE $PIPESTATUS $EUID $UID $HOSTNAME $SECONDS $BASH_VERSION ${FUNCNAME[0]}
echo $((RANDOM + SECONDS))
