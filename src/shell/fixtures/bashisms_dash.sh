#!/bin/dash
f() { local a; echo -n x; echo -e y; read -p p -r v; echo $_ ${x:0:1} $'a'; [[ a ]]; }
f
trap foo SIGINT
echo foo 42>/dev/null
echo $MACHTYPE
x='test'
${x:0:3}
x='test'
${x/st/xt}
$'a'
type -p test
